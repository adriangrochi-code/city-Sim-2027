//! Render con wgpu: pase de sombras del sol, cielo, vóxeles opacos, agua translúcida y una
//! capa de resaltado. Dibuja en una ventana (superficie) o en una textura para guardar
//! capturas sin pantalla.

mod alloc;

use crate::camera::OrbitCamera;
use crate::mesher::{Mesh, Quad};
use alloc::Buddy;
use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

const SHADOW_SIZE: u32 = 4096;
/// Vértices de resaltado por cuadro (vistas previas de calles y solares del modo juego).
pub const OVERLAY_MAX: usize = 196_608;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Columnas (orígenes de malla) que caben en las tablas de la GPU.
const ORIGIN_SLOTS: u32 = 65536;
/// Unidad del asignador del megabúfer de caras, en caras.
const QUAD_UNIT: u32 = 64;
const QUAD_BYTES: u64 = std::mem::size_of::<Quad>() as u64;
/// Capacidad inicial del megabúfer: 2^14 unidades = 1 M de caras (20 MB). Crece duplicándose.
const INITIAL_ORDER: u32 = 14;
/// Cuadros que la GPU puede tener en vuelo: un bloque liberado se reutiliza después de esto.
const FRAMES_IN_FLIGHT: u64 = 2;
/// Paleta de colores de material en la GPU (vec4 por material).
const PALETTE_SLOTS: usize = 64;
/// Los pases de sombra solo dibujan los niveles finos.
const SHADOW_MAX_LOD: u32 = 3;

/// Modo de dibujo elegido por línea de comandos (0 = automático, 1 = directo,
/// 2 = indirecto con descarte en CPU, 3 = indirecto con descarte en GPU).
pub static DRAW_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Cómo se emiten las llamadas de dibujo de los vóxeles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawMode {
    /// Una llamada `draw` por tramo de caras visible (descarte en la CPU).
    Direct,
    /// Descarte en la CPU, todos los tramos en un solo `multi_draw_indirect` (Vulkan).
    Indirect,
    /// Descarte en la GPU con un compute shader que llena los argumentos indirectos (Vulkan).
    GpuCull,
}

impl DrawMode {
    pub fn name(&self) -> &'static str {
        match self {
            DrawMode::Direct => "directo",
            DrawMode::Indirect => "indirecto múltiple (descarte en CPU)",
            DrawMode::GpuCull => "indirecto múltiple (descarte en GPU)",
        }
    }
}

/// Columna en la tabla de la GPU (64 bytes, igual que `Col` en cull.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ColGpu {
    lo: [f32; 4],
    hi: [f32; 4],
    start: u32,
    g: [u32; 7],
}

/// Parámetros del descarte en GPU (128 bytes, igual que `Params` en cull.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CullParams {
    planes: [[f32; 4]; 6],
    eye: [f32; 4],
    count: u32,
    shadow: u32,
    max_lod: u32,
    _pad: u32,
}

/// Argumentos de dibujo indexado indirecto (20 bytes, como `VkDrawIndexedIndirectCommand`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

fn draw_args(first: u32, count: u32) -> DrawArgs {
    DrawArgs { index_count: 6, instance_count: count, first_index: 0, base_vertex: 0, first_instance: first }
}

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
    /// Bloque del megabúfer: primera cara y orden del bloque del asignador.
    start: u32,
    order: u32,
    /// Dónde empieza cada grupo de caras (por dirección), relativo a `start`.
    groups: [u32; 7],
    quads: u32,
    lod: u32,
    /// Posición de esta columna en las tablas de la GPU (origen y descarte).
    slot: u32,
    lo: Vec3,
    hi: Vec3,
}

/// Planos de un volumen de visión (frustum) a partir de una matriz de proyección × vista
/// (profundidad de 0 a 1, como en wgpu).
fn frustum_planes(m: Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2]
}

/// ¿La caja (lo, hi) toca el volumen de visión?
fn box_visible(planes: &[Vec4; 6], lo: Vec3, hi: Vec3) -> bool {
    for p in planes {
        let v = Vec3::new(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
        if p.x * v.x + p.y * v.y + p.z * v.z + p.w < 0.0 {
            return false;
        }
    }
    true
}

/// Qué grupos de caras pueden verse desde `eye` para una caja: las caras que miran en
/// sentido contrario a la cámara no se dibujan (orden de [`crate::mesher::DIRS`]).
fn visible_dirs(eye: Vec3, lo: Vec3, hi: Vec3) -> [bool; 6] {
    [eye.y > lo.y, eye.x > lo.x, eye.x < hi.x, eye.z > lo.z, eye.z < hi.z, eye.y < hi.y]
}

/// Grupos de caras que miran al sol (`sun` apunta hacia el sol), orden de [`crate::mesher::DIRS`].
fn sun_dirs(sun: Vec3) -> [bool; 6] {
    [sun.y > 0.0, sun.x > 0.0, sun.x < 0.0, sun.z > 0.0, sun.z < 0.0, sun.y < 0.0]
}

/// Medición de un cuadro para el banco de pruebas.
#[derive(Clone, Copy, Default, Debug)]
pub struct FrameStats {
    /// Tiempo de CPU en preparar y grabar el cuadro (globales + comandos), en ms.
    pub cpu_ms: f64,
    /// Tiempo total hasta que el cuadro terminó de dibujarse (con lavapipe, render por CPU), en ms.
    pub total_ms: f64,
    pub draws_main: u32,
    pub draws_shadow: u32,
    /// Tiempo de GPU del cuadro medido con marcas de tiempo (si el adaptador las tiene), en ms.
    pub gpu_ms: Option<f64>,
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
    origins: wgpu::Buffer,
    shadow_vox_bg: wgpu::BindGroup,
    /// Megabúfer con las caras de todas las columnas y su asignador por bloques.
    quads: wgpu::Buffer,
    /// Índices compartidos de una cara (0, 1, 2, 0, 2, 3).
    quad_index: wgpu::Buffer,
    alloc: Buddy,
    frame: u64,
    /// Tabla de columnas para el descarte en GPU y sus argumentos de dibujo.
    cols: wgpu::Buffer,
    args_main: wgpu::Buffer,
    args_shadow: wgpu::Buffer,
    args_cpu: wgpu::Buffer,
    cull_pipe: Option<wgpu::ComputePipeline>,
    cull_params_main: wgpu::Buffer,
    cull_params_shadow: wgpu::Buffer,
    cull_bg_main: wgpu::BindGroup,
    cull_bg_shadow: wgpu::BindGroup,
    pub draw_mode: DrawMode,
    pub can_indirect: bool,
    timestamps: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer, f32)>,
    /// Bytes de caras subidos (acumulado; el título y el banco de pruebas muestran la diferencia).
    pub uploaded_bytes: u64,
    /// Sube con cada cambio de geometría o de resaltado (parte de la clave del cuadro).
    pub scene_version: u64,
    overlay_key: u64,
    /// Llamadas de dibujo del último cuadro de la ventana (principal, sombras).
    pub last_draws: (u32, u32),
    /// Bytes de caras subidos en el último cuadro de la ventana.
    pub upload_frame: u64,
    upload_mark: u64,
    free_slots: Vec<u32>,
    next_slot: u32,
    batches: Vec<Batch>,
    columns: HashMap<(u32, IVec2), Batch, crate::fxhash::FxBuild>,
    overlay: wgpu::Buffer,
    overlay_count: u32,
    targets: Option<Targets>,
    bench_out: Option<(u32, u32, wgpu::TextureView)>,
    /// Caché del mapa de sombras: se rehace solo si cambió la luz o la geometría.
    shadow_key: std::cell::Cell<Option<[f32; 16]>>,
    shadow_dirty: std::cell::Cell<bool>,
    /// Planos de la caja de luz del último mapa de sombras: un cambio de geometría solo
    /// invalida la sombra si cae dentro (clave de sombra, técnica de Tisma engine).
    shadow_planes: std::cell::Cell<Option<[Vec4; 6]>>,
    /// Veces que se rehízo el mapa de sombras (para el banco de pruebas).
    pub shadow_redraws: std::cell::Cell<u32>,
    pub adapter_name: String,
    /// Suaviza el sombreado de los escalones del terreno (evita las líneas oscuras de lejos).
    pub smooth_terrain: bool,
    /// Tiempo en segundos (para las ondas del agua).
    pub time: f32,
}

/// Matrices del cuadro, para descartar lo que no se ve.
struct View {
    eye: Vec3,
    sun: Vec3,
    view_proj: Mat4,
    light_vp: Mat4,
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

fn storage_entry(binding: u32, visibility: wgpu::ShaderStages, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

fn make_quad_buffer(device: &wgpu::Device, units: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("caras"),
        size: units * QUAD_UNIT as u64 * QUAD_BYTES,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
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
        Self::new_msaa(4)
    }

    /// Renderer sin ventana con el antialiasing indicado (1 = sin MSAA).
    pub fn new_msaa(msaa: u32) -> Result<Renderer, String> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::all(), ..Default::default() });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: None })
                .await
                .ok_or("no se encontró ningún adaptador gráfico (¿falta un driver Vulkan?)")?;
            Self::from_adapter(&adapter, OFFSCREEN_FORMAT, msaa).await
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
            // MSAA automático (0): con render por software (lavapipe, llvmpipe…) el 4× cuesta
            // ~40 % del cuadro, así que se usa 1×; con GPU, 4×.
            let msaa = match msaa {
                0 if adapter.get_info().device_type == wgpu::DeviceType::Cpu => 1,
                0 => 4,
                m => m,
            };
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
        // Funciones de Vulkan que se usan si el adaptador las tiene: dibujo indirecto múltiple
        // (con primera instancia) y marcas de tiempo de la GPU.
        let wanted = wgpu::Features::MULTI_DRAW_INDIRECT | wgpu::Features::INDIRECT_FIRST_INSTANCE | wgpu::Features::TIMESTAMP_QUERY;
        let features = adapter.features() & wanted;
        let can_indirect = features.contains(wgpu::Features::MULTI_DRAW_INDIRECT | wgpu::Features::INDIRECT_FIRST_INSTANCE);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("motor"), required_features: features, required_limits: device_limits(adapter), memory_hints: Default::default() }, None)
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
                storage_entry(3, wgpu::ShaderStages::VERTEX, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("sombras"), entries: &[uniform_entry, storage_entry(3, wgpu::ShaderStages::VERTEX, true)] });
        let origins = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("orígenes"),
            size: ORIGIN_SLOTS as u64 * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut pal = vec![[0f32; 4]; PALETTE_SLOTS];
        for (m, c) in pal.iter_mut().enumerate().take(crate::palette::COUNT) {
            let rgb = crate::palette::color(m as u8);
            *c = [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0];
        }
        let palette = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("paleta"), contents: bytemuck::cast_slice(&pal), usage: wgpu::BufferUsages::UNIFORM });
        let simple_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("solo globales"), entries: &[uniform_entry] });
        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: origins.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: palette.as_entire_binding() },
            ],
        });
        let shadow_vox_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }, wgpu::BindGroupEntry { binding: 3, resource: origins.as_entire_binding() }],
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
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&shadow_bgl], push_constant_ranges: &[] });

        // Una instancia por cara compacta (mesher::Quad, 20 bytes); el shader arma los vértices.
        let quad_layout = wgpu::VertexBufferLayout {
            array_stride: QUAD_BYTES,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Uint32, 1 => Uint32, 2 => Uint32x2, 3 => Uint32],
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
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_main", compilation_options: Default::default(), buffers: &[quad_layout.clone()] },
            fragment: Some(wgpu::FragmentState { module: &voxel_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(reverse_depth(true, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sombras"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_shadow", compilation_options: Default::default(), buffers: &[quad_layout] },
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
        let alloc = Buddy::new(INITIAL_ORDER);
        let quad_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("índices de cara"), contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3, 0, 0]), usage: wgpu::BufferUsages::INDEX });
        let quads = make_quad_buffer(&device, alloc.capacity());
        let buf = |label: &str, size: u64, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let cols = buf("columnas", ORIGIN_SLOTS as u64 * 64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let args_size = ORIGIN_SLOTS as u64 * 6 * std::mem::size_of::<DrawArgs>() as u64;
        let args_main = buf("args principal", args_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT);
        let args_shadow = buf("args sombras", args_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT);
        let args_cpu = buf("args cpu", args_size * 2, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST);
        let cull_params_main = buf("descarte principal", 128, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let cull_params_shadow = buf("descarte sombras", 128, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let cull_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("descarte"),
            entries: &[
                storage_entry(0, wgpu::ShaderStages::COMPUTE, true),
                storage_entry(1, wgpu::ShaderStages::COMPUTE, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let cull_bg = |args: &wgpu::Buffer, params: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &cull_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: cols.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: args.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
                ],
            })
        };
        let cull_bg_main = cull_bg(&args_main, &cull_params_main);
        let cull_bg_shadow = cull_bg(&args_shadow, &cull_params_shadow);
        let cull_pipe = if can_indirect {
            let module = shader("descarte", include_str!("shaders/cull.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&cull_bgl], push_constant_ranges: &[] });
            Some(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("descarte"), layout: Some(&layout), module: &module, entry_point: "main", compilation_options: Default::default(), cache: None }))
        } else {
            None
        };
        let timestamps = if features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("tiempos"), ty: wgpu::QueryType::Timestamp, count: 2 });
            let resolve = buf("tiempos", 16, wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
            let read = buf("tiempos lectura", 16, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
            Some((set, resolve, read, queue.get_timestamp_period()))
        } else {
            None
        };
        // Descarte en GPU por defecto solo con GPU real; con render por CPU (lavapipe) se usa el
        // indirecto con descarte en CPU, que mantiene el orden de cerca a lejos.
        let auto = if !can_indirect {
            DrawMode::Direct
        } else if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            DrawMode::Indirect
        } else {
            DrawMode::GpuCull
        };
        let draw_mode = match DRAW_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => DrawMode::Direct,
            2 if can_indirect => DrawMode::Indirect,
            3 if can_indirect => DrawMode::GpuCull,
            0 => auto,
            _ => {
                eprintln!("el adaptador no tiene dibujo indirecto múltiple: se usa el modo directo");
                DrawMode::Direct
            }
        };
        let overlay = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resaltado"),
            size: (std::mem::size_of::<OverlayVertex>() * OVERLAY_MAX) as u64,
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
            origins,
            shadow_vox_bg,
            quads,
            quad_index,
            alloc,
            frame: 0,
            cols,
            args_main,
            args_shadow,
            args_cpu,
            cull_pipe,
            cull_params_main,
            cull_params_shadow,
            cull_bg_main,
            cull_bg_shadow,
            draw_mode,
            can_indirect,
            timestamps,
            uploaded_bytes: 0,
            scene_version: 0,
            overlay_key: 0,
            last_draws: (0, 0),
            upload_frame: 0,
            upload_mark: 0,
            free_slots: Vec::new(),
            next_slot: 0,
            batches: Vec::new(),
            columns: HashMap::default(),
            overlay,
            overlay_count: 0,
            targets: None,
            bench_out: None,
            shadow_key: std::cell::Cell::new(None),
            shadow_dirty: std::cell::Cell::new(true),
            shadow_planes: std::cell::Cell::new(None),
            shadow_redraws: std::cell::Cell::new(0),
            adapter_name,
            smooth_terrain: true,
            time: 3.0,
        })
    }

    /// Sube mallas sueltas a la GPU (modo captura: cada malla con su origen).
    pub fn upload(&mut self, meshes: &[(u32, Mesh)]) {
        for (lod, m) in meshes {
            if let Some(b) = self.make_batch(m, *lod) {
                self.batches.push(b);
            }
        }
    }

    fn alloc_slot(&mut self) -> u32 {
        if let Some(s) = self.free_slots.pop() {
            return s;
        }
        let s = self.next_slot;
        assert!(s < ORIGIN_SLOTS, "demasiadas mallas a la vez para el búfer de orígenes");
        self.next_slot += 1;
        s
    }

    fn free_batch(&mut self, b: Batch) {
        self.scene_version += 1;
        // La columna deja de dibujarse en la GPU y su bloque se libera con retraso.
        self.queue.write_buffer(&self.cols, b.slot as u64 * 64, bytemuck::bytes_of(&ColGpu::zeroed()));
        self.alloc.free_later(self.frame, b.start / QUAD_UNIT, b.order);
        self.free_slots.push(b.slot);
    }

    /// Reserva un bloque del megabúfer; si no hay lugar, lo agranda al doble (copiando).
    fn alloc_quads(&mut self, n: u32) -> (u32, u32) {
        let units = n.div_ceil(QUAD_UNIT);
        loop {
            if let Some((off, order)) = self.alloc.alloc(units) {
                return (off * QUAD_UNIT, order);
            }
            let old_bytes = self.quads.size();
            self.alloc.grow();
            let bigger = make_quad_buffer(&self.device, self.alloc.capacity());
            let mut enc = self.device.create_command_encoder(&Default::default());
            enc.copy_buffer_to_buffer(&self.quads, 0, &bigger, 0, old_bytes);
            self.queue.submit([enc.finish()]);
            self.quads = bigger;
        }
    }

    fn make_batch(&mut self, m: &Mesh, lod: u32) -> Option<Batch> {
        self.scene_version += 1;
        self.shadow_dirty_if(m);
        if m.is_empty() {
            return None;
        }
        let (mut quads, groups) = m.quads_grouped();
        let slot = self.alloc_slot();
        for q in quads.iter_mut() {
            q.slot = slot;
        }
        let (start, order) = self.alloc_quads(quads.len() as u32);
        let bytes: &[u8] = bytemuck::cast_slice(&quads);
        self.queue.write_buffer(&self.quads, start as u64 * QUAD_BYTES, bytes);
        self.uploaded_bytes += bytes.len() as u64;
        let origin = [m.origin.x, m.origin.y, m.origin.z, m.voxel];
        self.queue.write_buffer(&self.origins, slot as u64 * 16, bytemuck::cast_slice(&origin));
        let (lo, hi) = m.bounds();
        let col = ColGpu { lo: [lo.x, lo.y, lo.z, lod as f32], hi: [hi.x, hi.y, hi.z, 1.0], start, g: [groups[0], groups[1], groups[2], groups[3], groups[4], groups[5], groups[6]] };
        self.queue.write_buffer(&self.cols, slot as u64 * 64, bytemuck::bytes_of(&col));
        Some(Batch { start, order, groups, quads: quads.len() as u32, lod, slot, lo, hi })
    }

    /// Un cambio de geometría solo obliga a rehacer la sombra si cae dentro de la caja de luz
    /// del último mapa (o si es de un nivel que no proyecta sombra, no la toca).
    fn shadow_dirty_if(&self, m: &Mesh) {
        if m.is_empty() {
            return;
        }
        let (lo, hi) = m.bounds();
        match self.shadow_planes.get() {
            Some(p) if !box_visible(&p, lo, hi) => {}
            _ => self.shadow_dirty.set(true),
        }
    }

    /// Reemplaza la malla de una columna del mundo en streaming (vacía = la quita).
    pub fn set_column(&mut self, key: (u32, IVec2), mesh: &Mesh) {
        let new = self.make_batch(mesh, key.0);
        let old = match new {
            Some(b) => self.columns.insert(key, b),
            None => self.columns.remove(&key),
        };
        if let Some(b) = old {
            if self.shadow_planes.get().map_or(true, |p| box_visible(&p, b.lo, b.hi)) {
                self.shadow_dirty.set(true);
            }
            self.free_batch(b);
        }
    }

    pub fn remove_column(&mut self, key: (u32, IVec2)) {
        if let Some(b) = self.columns.remove(&key) {
            if self.shadow_planes.get().map_or(true, |p| box_visible(&p, b.lo, b.hi)) {
                self.shadow_dirty.set(true);
            }
            self.free_batch(b);
        }
    }

    /// Avanza el contador de cuadros y libera los bloques que la GPU ya no usa.
    pub fn maintain(&mut self) {
        // Un envío (aunque esté vacío) hace que wgpu ejecute las copias pendientes de las
        // mallas nuevas y libere sus búferes intermedios; sin ventana casi no hay envíos.
        self.queue.submit(std::iter::empty());
        self.device.poll(wgpu::Maintain::Poll);
        self.frame += 1;
        self.alloc.collect(self.frame, FRAMES_IN_FLIGHT);
    }

    /// Memoria de las mallas en la GPU (bytes de caras realmente usados), en MB.
    pub fn mesh_mb(&self) -> f64 {
        self.quad_count() as f64 * QUAD_BYTES as f64 / 1048576.0
    }

    /// Tamaño del megabúfer de caras (reservado), en MB.
    pub fn megabuffer_mb(&self) -> f64 {
        self.quads.size() as f64 / 1048576.0
    }

    /// Obliga a rehacer el mapa de sombras en el próximo cuadro.
    pub fn invalidate_shadows(&self) {
        self.shadow_dirty.set(true);
    }

    /// Quita todas las mallas (banco de pruebas).
    pub fn clear_columns(&mut self) {
        self.shadow_dirty.set(true);
        self.columns.clear();
        self.batches.clear();
        self.free_slots.clear();
        self.next_slot = 0;
        self.alloc = Buddy::new(INITIAL_ORDER);
        self.quads = make_quad_buffer(&self.device, self.alloc.capacity());
        let zeros = vec![0u8; ORIGIN_SLOTS as usize * 64];
        self.queue.write_buffer(&self.cols, 0, &zeros);
    }

    pub fn quad_count(&self) -> usize {
        self.columns.values().chain(self.batches.iter()).map(|b| b.quads as usize).sum()
    }

    /// Bytes de caras e índices en la GPU (ya no hay búfer de índices).
    pub fn gpu_bytes(&self) -> (u64, u64) {
        (self.quad_count() as u64 * QUAD_BYTES, 0)
    }

    /// Dibuja un cuadro fuera de pantalla y mide CPU y tiempo total (banco de pruebas).
    pub fn bench_frame(&mut self, cam: &OrbitCamera, hour: f32, width: u32, height: u32) -> FrameStats {
        if self.targets.as_ref().map(|t| (t.width, t.height)) != Some((width, height)) {
            self.targets = Some(self.make_targets(width, height));
        }
        if self.bench_out.as_ref().map(|b| (b.0, b.1)) != Some((width, height)) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.bench_out = Some((width, height, tex.create_view(&Default::default())));
        }
        let t0 = std::time::Instant::now();
        let view = self.write_globals(cam, hour, width as f32 / height as f32);
        let mut enc = self.device.create_command_encoder(&Default::default());
        let ts = self.timestamps.as_ref().map(|t| &t.0);
        let (dm, ds) = self.encode(&mut enc, self.targets.as_ref().unwrap(), &self.bench_out.as_ref().unwrap().2, &view, ts);
        if let Some((set, resolve, read, _)) = &self.timestamps {
            enc.resolve_query_set(set, 0..2, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, read, 0, 16);
        }
        let cmd = enc.finish();
        let cpu = t0.elapsed();
        self.queue.submit([cmd]);
        self.device.poll(wgpu::Maintain::Wait);
        let total_ms = t0.elapsed().as_secs_f64() * 1000.0;
        // Tiempo de GPU del pase principal (marcas de tiempo; el banco ya esperó el cuadro).
        let gpu_ms = self.timestamps.as_ref().and_then(|(_, _, read, period)| {
            let slice = read.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            self.device.poll(wgpu::Maintain::Wait);
            let v: Vec<u64> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
            read.unmap();
            (v[1] > v[0]).then(|| (v[1] - v[0]) as f64 * *period as f64 / 1e6)
        });
        self.frame += 1;
        self.alloc.collect(self.frame, FRAMES_IN_FLIGHT);
        FrameStats { cpu_ms: cpu.as_secs_f64() * 1000.0, total_ms, draws_main: dm, draws_shadow: ds, gpu_ms }
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub fn triangles(&self) -> usize {
        self.quad_count() * 2
    }

    /// Cajas de resaltado para el próximo cuadro.
    pub fn set_overlay(&mut self, verts: &[OverlayVertex]) {
        let n = verts.len().min(OVERLAY_MAX);
        let bytes: &[u8] = bytemuck::cast_slice(&verts[..n]);
        let mut h = crate::fxhash::FxHasher::default();
        std::hash::Hasher::write(&mut h, bytes);
        let key = std::hash::Hasher::finish(&h);
        if key != self.overlay_key || n as u32 != self.overlay_count {
            if n > 0 {
                self.queue.write_buffer(&self.overlay, 0, bytes);
            }
            self.overlay_key = key;
            self.overlay_count = n as u32;
            self.scene_version += 1;
        }
    }

    /// Clave del cuadro (técnica de Tisma engine, `frame_key()` en apps/tisma/renderer.cpp):
    /// si no cambiaron la cámara, la hora, el tamaño ni la escena, el cuadro sería idéntico y
    /// no hace falta dibujarlo.
    pub fn frame_key(&self, cam: &OrbitCamera, hour: f32, size: (u32, u32)) -> [u64; 4] {
        let c = [cam.target.x, cam.target.y, cam.target.z, cam.yaw, cam.pitch, cam.distance, cam.fov_y, hour];
        let mut h = crate::fxhash::FxHasher::default();
        for v in c {
            std::hash::Hasher::write_u32(&mut h, v.to_bits());
        }
        [std::hash::Hasher::finish(&h), self.scene_version, size.0 as u64, size.1 as u64]
    }

    fn write_globals(&self, cam: &OrbitCamera, hour: f32, aspect: f32) -> View {
        let l = lighting(hour);
        let view_proj = cam.proj(aspect) * cam.view();
        // Cámara de sombras: ortográfica desde el sol, centrada en el objetivo.
        let half = (cam.distance * 0.9).clamp(120.0, 1500.0);
        let light_up = if l.sun_dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let light_view = Mat4::look_at_rh(cam.target + l.sun_dir * 3000.0, cam.target, light_up);
        let light_proj = Mat4::orthographic_rh(-half, half, -half, half, 10.0, 6000.0);
        // Se ajusta la caja de luz a la grilla de texels del mapa de sombras: las sombras no
        // "titilan" al mover la cámara y, si la cámara apenas se movió, el mapa no cambia.
        let raw = light_proj * light_view;
        let o = raw * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let texel = 2.0 / SHADOW_SIZE as f32;
        let snap = Mat4::from_translation(Vec3::new((o.x / texel).round() * texel - o.x, (o.y / texel).round() * texel - o.y, 0.0));
        let light_vp = snap * raw;
        let g = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            light_view_proj: light_vp.to_cols_array_2d(),
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
        View { eye: cam.eye(), sun: l.sun_dir, view_proj, light_vp }
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

    /// Tramos de caras a dibujar en la vista principal: descarte por frustum, orden de cerca a
    /// lejos y solo los grupos de caras que miran a la cámara, unidos en tramos contiguos
    /// (unión de rangos contiguos, como `preview_draws()` de Tisma engine).
    fn main_ranges(&self, view: &View) -> Vec<DrawArgs> {
        let vp = frustum_planes(view.view_proj);
        let mut list: Vec<(f32, &Batch)> = self
            .batches
            .iter()
            .chain(self.columns.values())
            .filter(|b| box_visible(&vp, b.lo, b.hi))
            .map(|b| (((b.lo + b.hi) * 0.5 - view.eye).length_squared(), b))
            .collect();
        list.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.lod.cmp(&b.1.lod))
                .then(a.1.lo.x.partial_cmp(&b.1.lo.x).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.1.lo.z.partial_cmp(&b.1.lo.z).unwrap_or(std::cmp::Ordering::Equal))
        });
        let mut out = Vec::with_capacity(list.len() * 2);
        for (_, b) in list {
            let vis = visible_dirs(view.eye, b.lo, b.hi);
            let mut g = 0;
            while g < 6 {
                if !vis[g] || b.groups[g] == b.groups[g + 1] {
                    g += 1;
                    continue;
                }
                let start = b.groups[g];
                while g < 6 && (vis[g] || b.groups[g] == b.groups[g + 1]) {
                    g += 1;
                }
                out.push(draw_args(b.start + start, b.groups[g] - start));
            }
        }
        out
    }

    /// Tramos del pase de sombras: niveles finos dentro de la caja de luz, y solo los grupos
    /// de caras que miran al sol. En volúmenes cerrados de vóxeles la superficie más cercana a
    /// la luz siempre es una cara que la mira, así que el mapa de profundidad no cambia.
    fn shadow_ranges(&self, light_vp: Mat4, sun: Vec3) -> Vec<DrawArgs> {
        let lp = frustum_planes(light_vp);
        let vis = sun_dirs(sun);
        let mut out = Vec::new();
        for b in self.batches.iter().chain(self.columns.values()).filter(|b| b.lod <= SHADOW_MAX_LOD && box_visible(&lp, b.lo, b.hi)) {
            let mut g = 0;
            while g < 6 {
                if !vis[g] || b.groups[g] == b.groups[g + 1] {
                    g += 1;
                    continue;
                }
                let start = b.groups[g];
                while g < 6 && (vis[g] || b.groups[g] == b.groups[g + 1]) {
                    g += 1;
                }
                out.push(draw_args(b.start + start, b.groups[g] - start));
            }
        }
        out
    }

    fn cull_params(&self, m: Mat4, eye: Vec3, shadow: bool) -> CullParams {
        let pl = frustum_planes(m);
        CullParams {
            planes: pl.map(|p| p.into()),
            eye: eye.extend(1.0).into(),
            count: self.next_slot,
            shadow: shadow as u32,
            max_lod: SHADOW_MAX_LOD,
            _pad: 0,
        }
    }

    /// Graba los pases de sombras y principal. `out` es la imagen final (o el destino del
    /// resolve si hay MSAA). Devuelve las llamadas de dibujo (principal, sombras).
    fn encode(&self, enc: &mut wgpu::CommandEncoder, targets: &Targets, out: &wgpu::TextureView, view: &View, timestamps: Option<&wgpu::QuerySet>) -> (u32, u32) {
        let (mut dm, mut ds) = (0u32, 0u32);
        let key = view.light_vp.to_cols_array();
        let redraw = self.shadow_dirty.get() || self.shadow_key.get() != Some(key);
        let gpu = self.draw_mode == DrawMode::GpuCull && self.cull_pipe.is_some();
        let slots = self.next_slot;
        if gpu && slots > 0 {
            // Descarte en la GPU: un compute shader llena los argumentos indirectos.
            self.queue.write_buffer(&self.cull_params_main, 0, bytemuck::bytes_of(&self.cull_params(view.view_proj, view.eye, false)));
            if redraw {
                // En el pase de sombras "eye" lleva la dirección del sol (w = 0).
                self.queue.write_buffer(&self.cull_params_shadow, 0, bytemuck::bytes_of(&self.cull_params(view.light_vp, view.sun, true)));
            }
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("descarte"), timestamp_writes: None });
            cp.set_pipeline(self.cull_pipe.as_ref().unwrap());
            cp.set_bind_group(0, &self.cull_bg_main, &[]);
            cp.dispatch_workgroups(slots.div_ceil(64), 1, 1);
            if redraw {
                cp.set_bind_group(0, &self.cull_bg_shadow, &[]);
                cp.dispatch_workgroups(slots.div_ceil(64), 1, 1);
            }
        }
        // Rangos de la CPU (modos directo e indirecto). En indirecto se escriben en un solo
        // búfer: primero los de sombra y después los principales.
        let shadow_cpu = if redraw && !gpu { self.shadow_ranges(view.light_vp, view.sun) } else { Vec::new() };
        let main_cpu = if !gpu { self.main_ranges(view) } else { Vec::new() };
        let indirect = self.draw_mode == DrawMode::Indirect && self.can_indirect;
        if indirect {
            let mut all = shadow_cpu.clone();
            all.extend_from_slice(&main_cpu);
            if !all.is_empty() {
                self.queue.write_buffer(&self.args_cpu, 0, bytemuck::cast_slice(&all));
            }
        }
        if redraw {
            self.shadow_dirty.set(false);
            self.shadow_key.set(Some(key));
            self.shadow_planes.set(Some(frustum_planes(view.light_vp)));
            self.shadow_redraws.set(self.shadow_redraws.get() + 1);
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
            pass.set_bind_group(0, &self.shadow_vox_bg, &[]);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
            if gpu {
                if slots > 0 {
                    pass.multi_draw_indexed_indirect(&self.args_shadow, 0, slots * 6);
                    ds = 1;
                }
            } else if indirect {
                if !shadow_cpu.is_empty() {
                    pass.multi_draw_indexed_indirect(&self.args_cpu, 0, shadow_cpu.len() as u32);
                    ds = 1;
                }
            } else {
                for a in &shadow_cpu {
                    pass.draw_indexed(0..6, 0, a.first_instance..a.first_instance + a.instance_count);
                    ds += 1;
                }
            }
        }
        let (view_tex, resolve) = match &targets.msaa {
            Some(m) => (m, Some(out)),
            None => (out, None),
        };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("principal"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: view_tex,
                resolve_target: resolve,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: timestamps.map(|q| wgpu::RenderPassTimestampWrites { query_set: q, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: Some(1) }),
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.sky_pipe);
        pass.set_bind_group(0, &self.shadow_bg, &[]);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.voxel_pipe);
        pass.set_bind_group(0, &self.main_bg, &[]);
        pass.set_vertex_buffer(0, self.quads.slice(..));
        pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
        if gpu {
            if slots > 0 {
                pass.multi_draw_indexed_indirect(&self.args_main, 0, slots * 6);
                dm = 1;
            }
        } else if indirect {
            if !main_cpu.is_empty() {
                let off = (shadow_cpu.len() * std::mem::size_of::<DrawArgs>()) as u64;
                pass.multi_draw_indexed_indirect(&self.args_cpu, off, main_cpu.len() as u32);
                dm = 1;
            }
        } else {
            for a in &main_cpu {
                pass.draw_indexed(0..6, 0, a.first_instance..a.first_instance + a.instance_count);
                dm += 1;
            }
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
        (dm, ds)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Dibuja un cuadro en la ventana; `ui` pinta la interfaz encima de la escena.
    pub fn render_surface(&mut self, surface: &wgpu::Surface<'static>, config: &wgpu::SurfaceConfiguration, cam: &OrbitCamera, hour: f32, ui: Option<UiHook>) -> Result<(), String> {
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
        let v = self.write_globals(cam, hour, w as f32 / h as f32);
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        let draws = self.encode(&mut enc, self.targets.as_ref().unwrap(), &view, &v, None);
        if let Some(ui) = ui {
            ui(&self.device, &self.queue, &mut enc, &view, (w, h));
        }
        self.last_draws = draws;
        self.upload_frame = self.uploaded_bytes - self.upload_mark;
        self.upload_mark = self.uploaded_bytes;
        self.queue.submit([enc.finish()]);
        frame.present();
        self.frame += 1;
        self.alloc.collect(self.frame, FRAMES_IN_FLIGHT);
        Ok(())
    }

    /// Dibuja la escena y la guarda como imagen (PNG o JPG según la extensión).
    pub fn capture(&self, cam: &OrbitCamera, hour: f32, width: u32, height: u32, path: &str) -> Result<(), String> {
        self.capture_with(cam, hour, width, height, path, None)
    }

    /// Como `capture`, con la interfaz pintada encima.
    pub fn capture_with(&self, cam: &OrbitCamera, hour: f32, width: u32, height: u32, path: &str, ui: Option<UiHook>) -> Result<(), String> {
        let v = self.write_globals(cam, hour, width as f32 / height as f32);
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
        self.encode(&mut enc, &targets, &resolve_v, &v, None);
        if let Some(ui) = ui {
            ui(&self.device, &self.queue, &mut enc, &resolve_v, (width, height));
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

/// Dibuja la interfaz encima de la escena: (dispositivo, cola, codificador, vista, tamaño).
pub type UiHook<'a> = &'a mut dyn FnMut(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView, (u32, u32));

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
