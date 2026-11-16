//! Render con wgpu: pase de sombras del sol, cielo, vóxeles opacos, agua translúcida y una
//! capa de resaltado. Dibuja en una ventana (superficie) o en una textura para guardar
//! capturas sin pantalla.

use crate::camera::OrbitCamera;
use crate::mesher::{Mesh, Vertex};
use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

const SHADOW_SIZE: u32 = 4096;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_VERTS_PER_BATCH: usize = 6_000_000;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    params: [f32; 4],
    extra: [f32; 4],
}

/// Vértice de la capa de resaltado.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

struct Batch {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
    lod: u32,
}

struct Targets {
    width: u32,
    height: u32,
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    msaa: u32,
    globals: wgpu::Buffer,
    main_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    voxel_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    water_pipe: wgpu::RenderPipeline,
    overlay_pipe: wgpu::RenderPipeline,
    water: wgpu::Buffer,
    batches: Vec<Batch>,
    columns: HashMap<(u32, IVec2), Batch>,
    overlay: wgpu::Buffer,
    overlay_count: u32,
    targets: Option<Targets>,
    pub adapter_name: String,
    /// Suaviza el sombreado de los escalones del terreno (evita las líneas oscuras de lejos).
    pub smooth_terrain: bool,
    /// Tiempo en segundos (para las ondas del agua).
    pub time: f32,
}

/// Luz del día según la hora (0–24). Sol del norte: estamos en el hemisferio sur.
pub struct Lighting {
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub sky_top: Vec3,
    pub sky_horizon: Vec3,
    pub night: f32,
    pub ambient: f32,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn srgb(r: f32, g: f32, b: f32) -> Vec3 {
    Vec3::new(r.powf(2.2), g.powf(2.2), b.powf(2.2))
}

pub fn lighting(hour: f32) -> Lighting {
    let ang = (hour / 24.0 - 0.25) * std::f32::consts::TAU;
    let elev = ang.sin();
    let day = smoothstep(-0.1, 0.3, elev);
    let sunset = (1.0 - elev.abs() / 0.28).clamp(0.0, 1.0);
    let (sun_dir, sun_color) = if elev >= 0.0 {
        let d = Vec3::new(ang.cos(), elev.max(0.02) * 0.9, -0.42).normalize();
        let c = srgb(1.0, 0.62, 0.38).lerp(srgb(1.0, 0.96, 0.9), smoothstep(0.05, 0.5, elev));
        (d, c * 2.6 * smoothstep(0.0, 0.18, elev))
    } else {
        let d = Vec3::new(-ang.cos(), (-elev).max(0.05) * 0.8, -0.3).normalize();
        (d, srgb(0.55, 0.65, 0.95) * 0.25 * smoothstep(0.0, 0.2, -elev))
    };
    let top = srgb(0.025, 0.035, 0.08).lerp(srgb(0.30, 0.48, 0.76), day);
    let mut horizon = srgb(0.05, 0.06, 0.11).lerp(srgb(0.74, 0.79, 0.85), day);
    horizon = horizon.lerp(srgb(0.98, 0.62, 0.40), sunset * 0.55);
    Lighting { sun_dir, sun_color, sky_top: top, sky_horizon: horizon, night: 1.0 - smoothstep(-0.06, 0.1, elev), ambient: 0.25 + 0.75 * day }
}

fn device_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    let mut limits = wgpu::Limits::default();
    limits.max_buffer_size = adapter.limits().max_buffer_size.min(1 << 30);
    limits.max_texture_dimension_2d = adapter.limits().max_texture_dimension_2d;
    limits
}

impl Renderer {
    /// Renderer sin ventana (captura a PNG).
    pub fn new() -> Result<Renderer, String> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::all(), ..Default::default() });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: None })
                .await
                .ok_or("no se encontró ningún adaptador gráfico (¿falta un driver Vulkan?)")?;
            Self::from_adapter(&adapter, OFFSCREEN_FORMAT, 4).await
        })
    }

    /// Renderer para una ventana: crea la superficie y elige su formato.
    pub fn for_window(window: Arc<winit::window::Window>, msaa: u32) -> Result<(Renderer, wgpu::Surface<'static>, wgpu::SurfaceConfiguration), String> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::all(), ..Default::default() });
            let size = window.inner_size();
            let surface = instance.create_surface(window).map_err(|e| format!("no se pudo crear la superficie: {e}"))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: Some(&surface) })
                .await
                .ok_or("ningún adaptador gráfico puede dibujar en esta ventana")?;
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
            let renderer = Self::from_adapter(&adapter, format, msaa).await?;
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            };
            surface.configure(&renderer.device, &config);
            Ok((renderer, surface, config))
        })
    }

    pub fn configure(&mut self, surface: &wgpu::Surface<'static>, config: &wgpu::SurfaceConfiguration) {
        surface.configure(&self.device, config);
        self.targets = None;
    }

    async fn from_adapter(adapter: &wgpu::Adapter, format: wgpu::TextureFormat, msaa: u32) -> Result<Renderer, String> {
        let adapter_name = format!("{} ({:?})", adapter.get_info().name, adapter.get_info().backend);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("motor"), required_features: wgpu::Features::empty(), required_limits: device_limits(adapter), memory_hints: Default::default() }, None)
            .await
            .map_err(|e| format!("no se pudo crear el dispositivo: {e}"))?;

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globales"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sombras"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&Default::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let main_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("principal"),
            entries: &[
                uniform_entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let simple_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("solo globales"), entries: &[uniform_entry] });
        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &simple_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let shader = |label, src: &str| device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let voxel_mod = shader("voxel", include_str!("shaders/voxel.wgsl"));
        let sky_mod = shader("cielo", include_str!("shaders/sky.wgsl"));
        let water_mod = shader("agua", include_str!("shaders/water.wgsl"));
        let overlay_mod = shader("resaltado", include_str!("shaders/overlay.wgsl"));
        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&main_bgl], push_constant_ranges: &[] });
        let simple_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&simple_bgl], push_constant_ranges: &[] });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Snorm8x4, 3 => Snorm8x4, 4 => Float32],
        };
        let ms = wgpu::MultisampleState { count: msaa, mask: !0, alpha_to_coverage_enabled: false };
        let reverse_depth = |write: bool, compare| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: compare,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let blend_target = Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL });

        let voxel_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxeles"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_main", compilation_options: Default::default(), buffers: &[vertex_layout.clone()] },
            fragment: Some(wgpu::FragmentState { module: &voxel_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(reverse_depth(true, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sombras"),
            layout: Some(&simple_layout),
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_shadow", compilation_options: Default::default(), buffers: &[vertex_layout] },
            fragment: None,
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cielo"),
            layout: Some(&simple_layout),
            vertex: wgpu::VertexState { module: &sky_mod, entry_point: "vs_main", compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &sky_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: Default::default(),
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Always)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let water_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agua"),
            layout: Some(&simple_layout),
            vertex: wgpu::VertexState {
                module: &water_mod,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: 12, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x3] }],
            },
            fragment: Some(wgpu::FragmentState { module: &water_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[blend_target.clone()] }),
            primitive: Default::default(),
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let overlay_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("resaltado"),
            layout: Some(&simple_layout),
            vertex: wgpu::VertexState {
                module: &overlay_mod,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<OverlayVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState { module: &overlay_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[blend_target] }),
            primitive: Default::default(),
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Always)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        // Superficie del río: un plano enorme al nivel del mar (en cuadrícula para la precisión).
        let mut wv: Vec<[f32; 3]> = Vec::new();
        let (c, s, n) = (12000.0f32, 6000.0f32, 20);
        for j in 0..n {
            for i in 0..n {
                let x0 = c - s * n as f32 / 2.0 + i as f32 * s;
                let z0 = c - s * n as f32 / 2.0 + j as f32 * s;
                let (x1, z1) = (x0 + s, z0 + s);
                wv.extend_from_slice(&[[x0, 0.0, z0], [x0, 0.0, z1], [x1, 0.0, z1], [x0, 0.0, z0], [x1, 0.0, z1], [x1, 0.0, z0]]);
            }
        }
        let water = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("agua"), contents: bytemuck::cast_slice(&wv), usage: wgpu::BufferUsages::VERTEX });
        let overlay = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resaltado"),
            size: (std::mem::size_of::<OverlayVertex>() * 4096) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Renderer {
            device,
            queue,
            format,
            msaa,
            globals,
            main_bg,
            shadow_bg,
            shadow_view,
            voxel_pipe,
            shadow_pipe,
            sky_pipe,
            water_pipe,
            overlay_pipe,
            water,
            batches: Vec::new(),
            columns: HashMap::new(),
            overlay,
            overlay_count: 0,
            targets: None,
            adapter_name,
            smooth_terrain: true,
            time: 3.0,
        })
    }

    /// Sube las mallas a la GPU agrupadas en lotes grandes (pocas llamadas de dibujo).
    pub fn upload(&mut self, meshes: &[(u32, Mesh)]) {
        let mut by_lod: std::collections::BTreeMap<u32, Vec<&Mesh>> = Default::default();
        for (lod, m) in meshes {
            by_lod.entry(*lod).or_default().push(m);
        }
        for (lod, list) in by_lod {
            let mut acc = Mesh::default();
            for m in list {
                if acc.vertices.len() + m.vertices.len() > MAX_VERTS_PER_BATCH {
                    if let Some(b) = self.make_batch(&acc, lod) {
                        self.batches.push(b);
                    }
                    acc = Mesh::default();
                }
                acc.append(m);
            }
            if let Some(b) = self.make_batch(&acc, lod) {
                self.batches.push(b);
            }
        }
    }

    fn make_batch(&self, m: &Mesh, lod: u32) -> Option<Batch> {
        if m.indices.is_empty() {
            return None;
        }
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.vertices), usage: wgpu::BufferUsages::VERTEX });
        let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.indices), usage: wgpu::BufferUsages::INDEX });
        Some(Batch { vbuf, ibuf, count: m.indices.len() as u32, lod })
    }

    /// Reemplaza la malla de una columna del mundo en streaming (vacía = la quita).
    pub fn set_column(&mut self, key: (u32, IVec2), mesh: &Mesh) {
        match self.make_batch(mesh, key.0) {
            Some(b) => {
                self.columns.insert(key, b);
            }
            None => {
                self.columns.remove(&key);
            }
        }
    }

    pub fn remove_column(&mut self, key: (u32, IVec2)) {
        self.columns.remove(&key);
    }

    /// Libera los recursos de la GPU que ya no se usan (columnas reemplazadas o descargadas).
    pub fn maintain(&self) {
        // Un envío (aunque esté vacío) hace que wgpu ejecute las copias pendientes de las
        // mallas nuevas y libere sus búferes intermedios; sin ventana casi no hay envíos.
        self.queue.submit(std::iter::empty());
        self.device.poll(wgpu::Maintain::Poll);
    }

    /// Memoria de las mallas en la GPU, en MB.
    pub fn mesh_mb(&self) -> f64 {
        self.columns.values().chain(self.batches.iter()).map(|b| (b.vbuf.size() + b.ibuf.size()) as f64).sum::<f64>() / 1048576.0
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub fn triangles(&self) -> usize {
        self.columns.values().chain(self.batches.iter()).map(|b| b.count as usize / 3).sum()
    }

    /// Cajas de resaltado para el próximo cuadro.
    pub fn set_overlay(&mut self, verts: &[OverlayVertex]) {
        let n = verts.len().min(4096);
        if n > 0 {
            self.queue.write_buffer(&self.overlay, 0, bytemuck::cast_slice(&verts[..n]));
        }
        self.overlay_count = n as u32;
    }

    fn write_globals(&self, cam: &OrbitCamera, hour: f32, aspect: f32) {
        let l = lighting(hour);
        let view_proj = cam.proj(aspect) * cam.view();
        // Cámara de sombras: ortográfica desde el sol, centrada en el objetivo.
        let half = (cam.distance * 0.9).clamp(120.0, 1500.0);
        let light_up = if l.sun_dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let light_view = Mat4::look_at_rh(cam.target + l.sun_dir * 3000.0, cam.target, light_up);
        let light_proj = Mat4::orthographic_rh(-half, half, -half, half, 10.0, 6000.0);
        let g = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            light_view_proj: (light_proj * light_view).to_cols_array_2d(),
            cam_pos: cam.eye().extend(1.0).into(),
            sun_dir: l.sun_dir.extend(0.0).into(),
            sun_color: l.sun_color.extend(0.0).into(),
            sky_top: l.sky_top.extend(0.0).into(),
            sky_horizon: l.sky_horizon.extend(0.0).into(),
            // Niebla: más tenue cuanto más se aleja la cámara, para que la vista del mapa no se lave.
            params: Vec4::new(1.0 / (9000.0f32).max(cam.distance * 7.0), l.night, self.time, l.ambient).into(),
            extra: [if self.smooth_terrain { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
    }

    fn make_targets(&self, width: u32, height: u32) -> Targets {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let make = |format, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor { label: None, size, mip_level_count: 1, sample_count: self.msaa, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
                .create_view(&Default::default())
        };
        let msaa = if self.msaa > 1 { Some(make(self.format, wgpu::TextureUsages::RENDER_ATTACHMENT)) } else { None };
        Targets { width, height, msaa, depth: make(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT) }
    }

    /// Graba los pases de sombras y principal. `out` es la imagen final (o el destino del
    /// resolve si hay MSAA).
    fn encode(&self, enc: &mut wgpu::CommandEncoder, targets: &Targets, out: &wgpu::TextureView) {
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sombras"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.shadow_pipe);
            pass.set_bind_group(0, &self.shadow_bg, &[]);
            for b in self.batches.iter().chain(self.columns.values()).filter(|b| b.lod <= 3) {
                pass.set_vertex_buffer(0, b.vbuf.slice(..));
                pass.set_index_buffer(b.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..b.count, 0, 0..1);
            }
        }
        let (view, resolve) = match &targets.msaa {
            Some(m) => (m, Some(out)),
            None => (out, None),
        };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("principal"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: resolve,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.sky_pipe);
        pass.set_bind_group(0, &self.shadow_bg, &[]);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.voxel_pipe);
        pass.set_bind_group(0, &self.main_bg, &[]);
        for b in self.batches.iter().chain(self.columns.values()) {
            pass.set_vertex_buffer(0, b.vbuf.slice(..));
            pass.set_index_buffer(b.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..b.count, 0, 0..1);
        }
        pass.set_pipeline(&self.water_pipe);
        pass.set_bind_group(0, &self.shadow_bg, &[]);
        pass.set_vertex_buffer(0, self.water.slice(..));
        pass.draw(0..(self.water.size() / 12) as u32, 0..1);
        if self.overlay_count > 0 {
            pass.set_pipeline(&self.overlay_pipe);
            pass.set_vertex_buffer(0, self.overlay.slice(..));
            pass.draw(0..self.overlay_count, 0..1);
        }
    }

    /// Dibuja un cuadro en la ventana.
    pub fn render_surface(&mut self, surface: &wgpu::Surface<'static>, config: &wgpu::SurfaceConfiguration, cam: &OrbitCamera, hour: f32) -> Result<(), String> {
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.configure(surface, config);
                surface.get_current_texture().map_err(|e| format!("superficie: {e}"))?
            }
            Err(e) => return Err(format!("superficie: {e}")),
        };
        let (w, h) = (frame.texture.width(), frame.texture.height());
        if self.targets.as_ref().map(|t| (t.width, t.height)) != Some((w, h)) {
            self.targets = Some(self.make_targets(w, h));
        }
        self.write_globals(cam, hour, w as f32 / h as f32);
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.encode(&mut enc, self.targets.as_ref().unwrap(), &view);
        self.queue.submit([enc.finish()]);
        frame.present();
        Ok(())
    }

    /// Dibuja la escena y la guarda como imagen (PNG o JPG según la extensión).
    pub fn capture(&self, cam: &OrbitCamera, hour: f32, width: u32, height: u32, path: &str) -> Result<(), String> {
        self.write_globals(cam, hour, width as f32 / height as f32);
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let targets = self.make_targets(width, height);
        let resolve = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let resolve_v = resolve.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.encode(&mut enc, &targets, &resolve_v);
        let row = (width * 4).div_ceil(256) * 256;
        let out = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * height) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture { texture: &resolve, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyBuffer { buffer: &out, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) } },
            size,
        );
        self.queue.submit([enc.finish()]);
        let slice = out.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let bgra = matches!(self.format, wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm);
        let mut img = image::RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let i = (y * row + x * 4) as usize;
                let (r, b) = if bgra { (data[i + 2], data[i]) } else { (data[i], data[i + 2]) };
                img.put_pixel(x, y, image::Rgb([r, data[i + 1], b]));
            }
        }
        drop(data);
        out.unmap();
        img.save(path).map_err(|e| format!("no se pudo guardar {path}: {e}"))
    }
}

/// Caja translúcida (12 triángulos) entre dos esquinas, en metros.
pub fn overlay_box(out: &mut Vec<OverlayVertex>, a: Vec3, b: Vec3, color: [f32; 4]) {
    let c = |x: f32, y: f32, z: f32| OverlayVertex { pos: [x, y, z], color };
    let (x0, y0, z0, x1, y1, z1) = (a.x, a.y, a.z, b.x, b.y, b.z);
    let quads = [
        [c(x0, y1, z0), c(x1, y1, z0), c(x1, y1, z1), c(x0, y1, z1)],
        [c(x0, y0, z0), c(x0, y0, z1), c(x1, y0, z1), c(x1, y0, z0)],
        [c(x0, y0, z1), c(x0, y1, z1), c(x1, y1, z1), c(x1, y0, z1)],
        [c(x0, y0, z0), c(x1, y0, z0), c(x1, y1, z0), c(x0, y1, z0)],
        [c(x1, y0, z0), c(x1, y0, z1), c(x1, y1, z1), c(x1, y1, z0)],
        [c(x0, y0, z0), c(x0, y1, z0), c(x0, y1, z1), c(x0, y0, z1)],
    ];
    for q in quads {
        out.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]);
    }
}
