//! Render con wgpu: pase de sombras del sol, cielo, vóxeles opacos y agua translúcida.
//! Funciona sin ventana (captura a PNG) para poder verificarlo en servidores.

use crate::camera::OrbitCamera;
use crate::mesher::{Mesh, Vertex};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

const SHADOW_SIZE: u32 = 4096;
const MSAA: u32 = 4;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
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
}

struct Batch {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
    lod: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    globals: wgpu::Buffer,
    main_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    voxel_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    water_pipe: wgpu::RenderPipeline,
    water: wgpu::Buffer,
    batches: Vec<Batch>,
    pub adapter_name: String,
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

impl Renderer {
    pub fn new() -> Result<Renderer, String> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Renderer, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::all(), ..Default::default() });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: None })
            .await
            .ok_or("no se encontró ningún adaptador gráfico (¿falta un driver Vulkan?)")?;
        let adapter_name = format!("{} ({:?})", adapter.get_info().name, adapter.get_info().backend);
        let mut limits = wgpu::Limits::default();
        limits.max_buffer_size = adapter.limits().max_buffer_size.min(1 << 30);
        limits.max_texture_dimension_2d = adapter.limits().max_texture_dimension_2d;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("motor"), required_features: wgpu::Features::empty(), required_limits: limits, memory_hints: Default::default() }, None)
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

        let voxel_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("voxel"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/voxel.wgsl").into()) });
        let sky_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cielo"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()) });
        let water_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("agua"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/water.wgsl").into()) });
        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&main_bgl], push_constant_ranges: &[] });
        let simple_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&simple_bgl], push_constant_ranges: &[] });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Snorm8x4, 3 => Float32],
        };
        let ms = wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false };
        let reverse_depth = |write: bool, compare| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: compare,
            stencil: Default::default(),
            bias: Default::default(),
        };

        let voxel_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxeles"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_main", compilation_options: Default::default(), buffers: &[vertex_layout.clone()] },
            fragment: Some(wgpu::FragmentState { module: &voxel_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(COLOR_FORMAT.into())] }),
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
            fragment: Some(wgpu::FragmentState { module: &sky_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(COLOR_FORMAT.into())] }),
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
            fragment: Some(wgpu::FragmentState {
                module: &water_mod,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Greater)),
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

        Ok(Renderer { device, queue, globals, main_bg, shadow_bg, shadow_view, voxel_pipe, shadow_pipe, sky_pipe, water_pipe, water, batches: Vec::new(), adapter_name })
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
                    self.push_batch(&acc, lod);
                    acc = Mesh::default();
                }
                acc.append(m);
            }
            self.push_batch(&acc, lod);
        }
    }

    fn push_batch(&mut self, m: &Mesh, lod: u32) {
        if m.indices.is_empty() {
            return;
        }
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.vertices), usage: wgpu::BufferUsages::VERTEX });
        let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.indices), usage: wgpu::BufferUsages::INDEX });
        self.batches.push(Batch { vbuf, ibuf, count: m.indices.len() as u32, lod });
    }

    /// Dibuja la escena y la guarda como imagen (PNG o JPG según la extensión).
    pub fn capture(&self, cam: &OrbitCamera, hour: f32, width: u32, height: u32, path: &str) -> Result<(), String> {
        let l = lighting(hour);
        let aspect = width as f32 / height as f32;
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
            params: Vec4::new(1.0 / 9000.0, l.night, 3.0, l.ambient).into(),
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));

        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let make = |format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor { label: None, size, mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
        };
        let msaa = make(COLOR_FORMAT, MSAA, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = make(COLOR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let depth = make(DEPTH_FORMAT, MSAA, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let (msaa_v, resolve_v, depth_v) = (msaa.create_view(&Default::default()), resolve.create_view(&Default::default()), depth.create_view(&Default::default()));

        let mut enc = self.device.create_command_encoder(&Default::default());
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
            for b in self.batches.iter().filter(|b| b.lod <= 3) {
                pass.set_vertex_buffer(0, b.vbuf.slice(..));
                pass.set_index_buffer(b.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..b.count, 0, 0..1);
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("principal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_v,
                    resolve_target: Some(&resolve_v),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_v,
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
            for b in &self.batches {
                pass.set_vertex_buffer(0, b.vbuf.slice(..));
                pass.set_index_buffer(b.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..b.count, 0, 0..1);
            }
            pass.set_pipeline(&self.water_pipe);
            pass.set_bind_group(0, &self.shadow_bg, &[]);
            pass.set_vertex_buffer(0, self.water.slice(..));
            pass.draw(0..(self.water.size() / 12) as u32, 0..1);
        }
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
        let mut img = image::RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let i = (y * row + x * 4) as usize;
                img.put_pixel(x, y, image::Rgb([data[i], data[i + 1], data[i + 2]]));
            }
        }
        drop(data);
        out.unmap();
        img.save(path).map_err(|e| format!("no se pudo guardar {path}: {e}"))
    }
}
