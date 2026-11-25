//! Render con wgpu: pase de sombras del sol, cielo, vóxeles opacos, agua translúcida y una
//! capa de resaltado. Dibuja en una ventana (superficie) o en una textura para guardar
//! capturas sin pantalla.

mod alloc;
pub mod cascades;
pub mod escala;
pub mod hiz;

use crate::camera::OrbitCamera;
use crate::mesher::{Mesh, Quad};
use alloc::Buddy;
use cascades::{Placement, ViewFrame, MAX_CASCADES};
pub use cascades::ShadowQuality;
use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

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
const PALETTE_SLOTS: usize = 96;
const _: () = assert!(crate::palette::COUNT <= PALETTE_SLOTS, "la paleta no entra en la GPU");
/// Los pases de sombra solo dibujan los niveles finos: los de vóxel de hasta 4 m.
const SHADOW_MAX_LOD: u32 = {
    let mut l = 0;
    while crate::lod::BASE_VOXEL * ((1u32 << l) as f32) < 4.0 {
        l += 1;
    }
    l
};

/// Las cascadas grandes (más de 1 km de medio lado: cerros y montes lejanos) dibujan además
/// los niveles de 8 y 16 m.
const SHADOW_FAR_LOD: u32 = SHADOW_MAX_LOD + 2;
/// Cascadas que se pueden volver a dibujar en un cuadro de la ventana; las demás esperan (y se
/// usan con la matriz con la que se dibujaron). Una cascada que esperó 2 cuadros pasa adelante.
pub const SHADOW_BUDGET_WINDOW: usize = 2;

/// Descarte de lo tapado (Hi-Z): 0 = automático (si el modo de dibujo es GPU), 1 = no, 2 = sí.
pub static HIZ_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

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

/// [render] Subidas del *streaming* juntas, una vez por cuadro. Antes cada columna hacía
/// cuatro `queue.write_buffer` (caras, origen, visibilidad y fila de la tabla) y cada retirada
/// una más: en el vuelo del banco, ~370 000 llamadas, cada una con su búfer intermedio que wgpu
/// crea, rastrea y libera al enviar (el 40 % del trabajo del hilo principal). Ahora las caras se
/// copian directo a un `StagingBelt` (búferes intermedios reusados) y las tablas de columnas y
/// orígenes tienen un espejo en la CPU: al final del cuadro se suben solo los tramos tocados,
/// unidos si están cerca. La visibilidad la escribe también el descarte en la GPU, así que de
/// ella se escriben solo los lugares tocados, sin unir.
struct Subidas {
    belt: wgpu::util::StagingBelt,
    enc: Option<wgpu::CommandEncoder>,
    cols: Vec<ColGpu>,
    origins: Vec<[f32; 4]>,
    sucias: Vec<u32>,
    visibles: Vec<u32>,
    /// [render] Comienzo del fundido por lugar (ver `Renderer::fundido`), subido por tramos.
    fund: Vec<f32>,
    fund_sucias: Vec<u32>,
}

/// Lugares libres entre dos tocados de la tabla que todavía conviene subir juntos (copiar 64 B
/// de más cuesta menos que otra copia).
const SUBIDA_HUECO: u32 = 8;

impl Subidas {
    fn new() -> Subidas {
        Subidas { belt: wgpu::util::StagingBelt::new(1 << 20), enc: None, cols: vec![ColGpu::zeroed(); ORIGIN_SLOTS as usize], origins: vec![[0.0; 4]; ORIGIN_SLOTS as usize], sucias: Vec::new(), visibles: Vec::new(), fund: vec![0.0; ORIGIN_SLOTS as usize], fund_sucias: Vec::new() }
    }

    /// Lugar en el búfer intermedio para `size` bytes que van a `target` en `off`.
    fn escribir<'a>(&'a mut self, device: &wgpu::Device, target: &wgpu::Buffer, off: u64, size: u64) -> wgpu::BufferViewMut<'a> {
        let enc = self.enc.get_or_insert_with(|| device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("subidas") }));
        self.belt.write_buffer(enc, target, off, wgpu::BufferSize::new(size).expect("subida vacía"), device)
    }

    fn poner_col(&mut self, slot: u32, col: ColGpu, origin: Option<[f32; 4]>, visible: bool) {
        self.cols[slot as usize] = col;
        if let Some(o) = origin {
            self.origins[slot as usize] = o;
        }
        self.sucias.push(slot);
        if visible {
            self.visibles.push(slot);
        }
    }

    /// Tramos `[a, b]` de lugares; `hueco` = cuántos lugares sin tocar puede haber en el medio.
    fn tramos(v: &mut Vec<u32>, hueco: u32) -> Vec<(u32, u32)> {
        v.sort_unstable();
        v.dedup();
        let mut out: Vec<(u32, u32)> = Vec::new();
        for &s in v.iter() {
            match out.last_mut() {
                Some(t) if s <= t.1 + 1 + hueco => t.1 = s,
                _ => out.push((s, s)),
            }
        }
        v.clear();
        out
    }

    /// Envía lo juntado en el cuadro. Devuelve si hubo un envío.
    /// [render] Comienzo del fundido de un lugar.
    fn poner_fundido(&mut self, slot: u32, v: f32) {
        self.fund[slot as usize] = v;
        self.fund_sucias.push(slot);
    }

    #[allow(clippy::too_many_arguments)]
    fn enviar(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, cols: &wgpu::Buffer, origins: &wgpu::Buffer, visible: &wgpu::Buffer, fundido: &wgpu::Buffer) -> bool {
        if !self.fund_sucias.is_empty() {
            let mut v = std::mem::take(&mut self.fund_sucias);
            for (a, b) in Subidas::tramos(&mut v, SUBIDA_HUECO * 4) {
                let (a, b) = (a as usize, b as usize + 1);
                let f: Vec<f32> = self.fund[a..b].to_vec();
                self.escribir(device, fundido, a as u64 * 4, (b - a) as u64 * 4).copy_from_slice(bytemuck::cast_slice(&f));
            }
            self.fund_sucias = v;
        }
        if !self.sucias.is_empty() {
            let mut v = std::mem::take(&mut self.sucias);
            for (a, b) in Subidas::tramos(&mut v, SUBIDA_HUECO) {
                let (a, b) = (a as usize, b as usize + 1);
                let c: Vec<ColGpu> = self.cols[a..b].to_vec();
                let o: Vec<[f32; 4]> = self.origins[a..b].to_vec();
                self.escribir(device, cols, a as u64 * 64, (b - a) as u64 * 64).copy_from_slice(bytemuck::cast_slice(&c));
                self.escribir(device, origins, a as u64 * 16, (b - a) as u64 * 16).copy_from_slice(bytemuck::cast_slice(&o));
            }
            self.sucias = v;
        }
        if !self.visibles.is_empty() {
            let mut v = std::mem::take(&mut self.visibles);
            for (a, b) in Subidas::tramos(&mut v, 0) {
                let n = (b - a + 1) as usize;
                self.escribir(device, visible, a as u64 * 4, n as u64 * 4).copy_from_slice(bytemuck::cast_slice(&vec![1u32; n]));
            }
            self.visibles = v;
        }
        let Some(enc) = self.enc.take() else {
            return false;
        };
        self.belt.finish();
        queue.submit([enc.finish()]);
        self.belt.recall();
        true
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

/// Parámetros del descarte en GPU (208 bytes, igual que `Params` en cull.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CullParams {
    planes: [[f32; 4]; 6],
    eye: [f32; 4],
    count: u32,
    shadow: u32,
    max_lod: u32,
    /// 0 = solo volumen de visión, 1 y 2 = pasadas del Hi-Z (ver cull.wgsl).
    phase: u32,
    view_proj: [[f32; 4]; 4],
    /// Ancho y alto en píxeles, niveles de la pirámide, plano cercano.
    screen: [f32; 4],
}

/// Cascadas de sombra para los shaders (igual que `Sombras` en sombras.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SombrasGpu {
    vp: [[[f32; 4]; 4]; MAX_CASCADES],
    info: [[f32; 4]; MAX_CASCADES],
    misc: [f32; 4],
}

/// Estado de las cascadas entre cuadros: dónde se quiere cada una (con histéresis), con qué
/// lugar y sol está dibujado su mapa, si la geometría la ensució y cuántos cuadros esperó.
#[derive(Default)]
struct CascState {
    sun: Vec3,
    want: [Option<Placement>; MAX_CASCADES],
    drawn: [Option<(Placement, Vec3)>; MAX_CASCADES],
    dirty: [bool; MAX_CASCADES],
    planes: [Option<[Vec4; 6]>; MAX_CASCADES],
    waited: [u32; MAX_CASCADES],
    /// Rehacer todas en el próximo cuadro, sin límite (banco de pruebas, cambio de calidad).
    force: bool,
    /// Quedaron cascadas esperando (límite por cuadro): hay que dibujar otro cuadro aunque la
    /// cámara no se mueva.
    pending: bool,
}

/// Recursos del Hi-Z que dependen del tamaño de la imagen.
struct HizTargets {
    levels: u32,
    reduce_bg: wgpu::BindGroup,
    /// Uno por nivel desde el 1 (lee el anterior, escribe este).
    down_bg: Vec<wgpu::BindGroup>,
    sizes: Vec<(u32, u32)>,
    /// Descarte de la segunda pasada (lee la pirámide).
    cull_bg: wgpu::BindGroup,
}

struct HizPipes {
    reduce: wgpu::ComputePipeline,
    down: wgpu::ComputePipeline,
    reduce_bgl: wgpu::BindGroupLayout,
    down_bgl: wgpu::BindGroupLayout,
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
    /// [agua en vóxeles] Caras del agua: van en el bloque después de las del terreno (desde
    /// `groups[6]`), y se dibujan aparte, en el pase del agua.
    water: u32,
    lod: u32,
    /// [render] Entrando con fundido (no va en la pasada principal ni en el Hi-Z).
    entra: bool,
    /// Posición de esta columna en las tablas de la GPU (origen y descarte).
    slot: u32,
    lo: Vec3,
    hi: Vec3,
}

/// [render] Estado de una columna en la tabla de la GPU (`hi.w`, ver cull.wgsl): estable (en la
/// vista principal y en las sombras), entrando (solo en la pasada del fundido y en las sombras) y
/// saliendo (solo en la pasada del fundido).
const ESTABLE: f32 = 1.0;
const ENTRA: f32 = 2.0;
const SALE: f32 = 3.0;
/// [render] Duración del fundido entre niveles (s), como `FundidoNiveles` del mod.
pub const FUNDIDO_S: f32 = 0.4;

fn col_gpu(b: &Batch, estado: f32) -> ColGpu {
    let g = b.groups;
    ColGpu { lo: [b.lo.x, b.lo.y, b.lo.z, b.lod as f32], hi: [b.hi.x, b.hi.y, b.hi.z, estado], start: b.start, g: [g[0], g[1], g[2], g[3], g[4], g[5], g[6]] }
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

/// Radio de la Tierra × 2 (m): el vertex shader baja cada punto d²/2R respecto de la cámara
/// (`curved()` en voxel.wgsl).
pub const EARTH_2R: f32 = 12_742_000.0;

/// Caja de una columna tal como se dibuja: con la curvatura, la geometría baja hasta d²/2R
/// (d = distancia horizontal del ojo a la esquina más lejana). Sin esto, de lejos y con la
/// cámara baja, una columna cuya caja quedaba apenas por encima del borde de arriba de la
/// pantalla se descartaba aunque su terreno (más bajo) sí se veía.
fn curved_box(eye: Vec3, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let dx = (eye.x - lo.x).abs().max((eye.x - hi.x).abs());
    let dz = (eye.z - lo.z).abs().max((eye.z - hi.z).abs());
    let drop = (dx * dx + dz * dz) / EARTH_2R;
    (Vec3::new(lo.x, lo.y - drop, lo.z), hi)
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

/// Agrega los tramos contiguos de los grupos de caras visibles de una columna.
fn push_groups(out: &mut Vec<DrawArgs>, b: &Batch, vis: &[bool; 6]) {
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
    /// Cascadas de sombra dibujadas en este cuadro.
    pub shadow_passes: u32,
    /// Columnas y caras dibujadas en la vista principal (en modo GPU, solo si se pidió leerlas).
    pub cols_main: u32,
    pub quads_main: u64,
    /// De esas, las de la segunda pasada del Hi-Z (lo que no se vio en el cuadro anterior).
    pub quads_phase2: u64,
    /// Caras dibujadas en los pases de sombra.
    pub quads_shadow: u64,
    /// Se usó el descarte de lo tapado.
    pub hiz: bool,
    /// [render] Se dibujó el mapa de sombra de los personajes.
    pub agent_shadow: bool,
    /// [render] Columnas y caras de la pasada del fundido entre niveles.
    pub cols_fade: u32,
    pub quads_fade: u64,
}

struct Targets {
    /// Tamaño de la imagen de la escena (con escala de render, más chico que la salida).
    width: u32,
    height: u32,
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    hiz: Option<HizTargets>,
    /// [render] Tamaño de la salida, escala y SSAO con que se armaron (si cambian, se rehacen).
    out: (u32, u32),
    escala: u32,
    ssao: Option<SsaoTargets>,
    escalado: Option<EscaladoTargets>,
}

/// [render] Media resolución del SSAO lejano (oclusión y distancia) y sus grupos.
struct SsaoTargets {
    ao: wgpu::TextureView,
    size: (u32, u32),
    bg_ssao: wgpu::BindGroup,
    bg_apply: wgpu::BindGroup,
}

/// [render] Imagen de la escena a la escala de render, la realzada a la salida y sus grupos.
struct EscaladoTargets {
    color: wgpu::TextureView,
    full: wgpu::TextureView,
    bg_up: wgpu::BindGroup,
    bg_rcas: wgpu::BindGroup,
}

/// [render] Pipelines de las pasadas de pantalla (`post.wgsl`): SSAO lejano y escala de render.
struct PostPipes {
    ssao: wgpu::RenderPipeline,
    apply: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    rcas: wgpu::RenderPipeline,
    bgl_ssao: wgpu::BindGroupLayout,
    bgl_apply: wgpu::BindGroupLayout,
    bgl_tex: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Parámetros de pantalla de la escena (SSAO) y de la salida (realce).
    params_scene: wgpu::Buffer,
    params_out: wgpu::Buffer,
}

const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

impl PostPipes {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, msaa: u32) -> PostPipes {
        let src = include_str!("shaders/post.wgsl").replace("DEPTH_TEX", if msaa > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pasadas de pantalla"), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let uni = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let depth = wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: msaa > 1 }, count: None };
        let tex = |binding, filterable| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };
        let bgl_ssao = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("ssao"), entries: &[uni(0), uni(1), depth] });
        let bgl_apply = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("ssao aplicar"), entries: &[uni(0), uni(1), depth, tex(3, false)] });
        let bgl_tex = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("escala"),
            entries: &[uni(1), tex(4, true), wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }],
        });
        let pipe = |label, bgl: &wgpu::BindGroupLayout, entry, target: wgpu::ColorTargetState| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[bgl], push_constant_ranges: &[] });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &module, entry_point: "vs_full", compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: entry, compilation_options: Default::default(), targets: &[Some(target)] }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        // Multiplica lo ya dibujado por el factor de oclusión.
        let mult = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let ssao = pipe("ssao", &bgl_ssao, "fs_ssao", AO_FORMAT.into());
        let apply = pipe("ssao aplicar", &bgl_apply, "fs_ssao_apply", wgpu::ColorTargetState { format, blend: Some(mult), write_mask: wgpu::ColorWrites::ALL });
        let up = pipe("escala: realce", &bgl_tex, "fs_upscale", format.into());
        let rcas = pipe("escala: nitidez", &bgl_tex, "fs_rcas", format.into());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let buf = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        PostPipes { ssao, apply, up, rcas, bgl_ssao, bgl_apply, bgl_tex, sampler, params_scene: buf("pantalla de la escena"), params_out: buf("pantalla de salida") }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    msaa: u32,
    globals: wgpu::Buffer,
    main_bg: wgpu::BindGroup,
    main_bgl: wgpu::BindGroupLayout,
    shadow_bg: wgpu::BindGroup,
    shadow_sampler: wgpu::Sampler,
    palette: wgpu::Buffer,
    /// Luces de los faroles cerca de la cámara, en una grilla de celdas (ver [`luces`]).
    luces: wgpu::Buffer,
    /// [render] Distancia a lo que todavía falta subir por sector (ver `Streamer::sectores_faltan`).
    faltan: wgpu::Buffer,
    faltan_val: [f32; crate::stream::SECTORES],
    luces_key: u64,
    /// [render] Fundido entre niveles (técnicas del mod LOD §2.5): avance por lugar de columna
    /// (binding 9), las columnas que entran (clave, desde cuándo) y las que salen (su bloque,
    /// desde cuándo). Las dos se dibujan con `fade_pipe` (tramado con `discard`), en una pasada
    /// aparte: el pipeline principal sigue sin `discard`.
    fundido: wgpu::Buffer,
    entrantes: std::collections::VecDeque<((u32, IVec2), u32, f32)>,
    salientes: std::collections::VecDeque<(Batch, f32)>,
    reloj: std::time::Instant,
    /// [render] Reloj del fundido quieto (banco: `forzar_fundido`).
    reloj_fijo: Option<f32>,
    fade_pipe: wgpu::RenderPipeline,
    post: PostPipes,
    /// [render] Fundir al cambiar de nivel (opción "Fundido entre niveles"; si no, cambio seco).
    /// Apagado fuera de la ventana: las capturas y el banco no esperan fundidos.
    pub fundir: bool,
    /// [render] Oclusión ambiental en pantalla para lo lejano (§2.13).
    pub ssao: bool,
    /// [render] Escala de render en % (ver [`escala`]).
    pub escala: u32,
    /// Mapas de sombra: una capa por cascada (vista de todo el arreglo y una por capa).
    shadow_view: wgpu::TextureView,
    shadow_layers: Vec<wgpu::TextureView>,
    quality: ShadowQuality,
    /// Matrices de las cascadas para los shaders y para el pase de sombras (256 bytes c/u).
    sombras: wgpu::Buffer,
    cascade_vp: wgpu::Buffer,
    casc: std::cell::RefCell<CascState>,
    /// Cascadas que puede rehacer un cuadro de la ventana (la captura y el banco, todas).
    pub shadow_budget: usize,
    voxel_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    water_pipe: wgpu::RenderPipeline,
    /// [agua en vóxeles] Caras del agua de las columnas (mismo formato que las del terreno).
    water_quad_pipe: wgpu::RenderPipeline,
    overlay_pipe: wgpu::RenderPipeline,
    /// Flora baja en micro-vóxeles: piezas de cada columna, y el búfer de instancias que se
    /// rearma (perezoso) cuando cambian.
    detail_pipe: wgpu::RenderPipeline,
    detail_cols: HashMap<(u32, IVec2), Vec<crate::trees::Micro>, crate::fxhash::FxBuild>,
    detail_static: Vec<crate::trees::Micro>,
    detail_buf: std::cell::RefCell<Option<(wgpu::Buffer, u32)>>,
    detail_dirty: std::cell::Cell<bool>,
    /// Vecinos y vehículos: una instancia por caja (`city::agentes::dibujo::Caja`), rehecho
    /// en cada cuadro.
    agent_pipe: wgpu::RenderPipeline,
    agent_shadow_pipe: wgpu::RenderPipeline, // [personajes]
    /// [render] Sombra proyectada de los personajes y animales: un mapa propio con la matriz de
    /// la cascada 0, rehecho en cada cuadro con solo las cajas de los agentes (las cascadas son
    /// estáticas y se guardan en caché: moverlas con cada paso las rehacía siempre). Lo leen
    /// `shadow()` (sombras.wgsl) en el terreno, la flora y los mismos agentes.
    agent_map: wgpu::TextureView,
    agent_vp: wgpu::Buffer,
    agent_cast_bg: wgpu::BindGroup,
    agent_cast_pipe: wgpu::RenderPipeline,
    /// [render] Personajes que proyectan sombra (opción; el banco la apaga para medir).
    pub sombra_agentes: bool,
    agent_buf: Option<wgpu::Buffer>,
    agent_count: u32,
    agent_key: u64,
    /// [render] Firma (cajas y matriz) con que se dibujó el mapa de sombra de los personajes.
    agent_shadow_key: std::cell::Cell<u64>,
    water: wgpu::Buffer,
    /// Agua de los arroyos (cintas sobre el cauce), si el mapa las tiene.
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
    args_main2: wgpu::Buffer,
    args_shadow: Vec<wgpu::Buffer>,
    args_cpu: wgpu::Buffer,
    /// [agua en vóxeles] Un dibujo por columna con agua, en un solo dibujo indirecto múltiple.
    args_agua: wgpu::Buffer,
    cull_pipe: Option<wgpu::ComputePipeline>,
    cull_bgl: wgpu::BindGroupLayout,
    cull_params_main: wgpu::Buffer,
    cull_params_main2: wgpu::Buffer,
    cull_params_shadow: Vec<wgpu::Buffer>,
    cull_bg_main: wgpu::BindGroup,
    cull_bg_shadow: Vec<wgpu::BindGroup>,
    /// Marca por columna: se vio en el último cuadro con Hi-Z (primera pasada).
    visible: wgpu::Buffer,
    hiz_pipes: Option<HizPipes>,
    /// El adaptador puede hacer el Hi-Z (compute, texturas de almacenamiento R32Float).
    pub hiz_supported: bool,
    /// Descarte de lo tapado activado (solo actúa en el modo de dibujo GPU).
    pub hiz: bool,
    pub draw_mode: DrawMode,
    pub can_indirect: bool,
    timestamps: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer, f32)>,
    /// [lod] Lectura asíncrona de las marcas de tiempo en la ventana (0 libre, 1 esperando el
    /// mapeo, 2 lista para leer) y el último tiempo de GPU del pase principal medido así.
    gpu_read: std::sync::Arc<std::sync::atomic::AtomicU8>,
    pub last_gpu_ms: Option<f64>,
    /// [lod] Espera de la superficie en el último cuadro de la ventana (tomar la imagen y
    /// presentarla: ahí se espera el vsync), en ms. El trabajo del cuadro es el resto.
    pub last_wait_ms: f64,
    /// Veces que se rehicieron los destinos de la escena (cambio de tamaño, de escala o de SSAO).
    pub destinos_rehechos: u32,
    /// Bytes de caras subidos (acumulado; el título y el banco de pruebas muestran la diferencia).
    pub uploaded_bytes: u64,
    /// Sube con cada cambio de geometría o de resaltado (parte de la clave del cuadro).
    pub scene_version: u64,
    overlay_key: u64,
    /// Llamadas de dibujo del último cuadro de la ventana (principal, sombras).
    pub last_draws: (u32, u32),
    /// Cascadas de sombra dibujadas en el último cuadro de la ventana.
    pub last_shadow_passes: u32,
    /// Bytes de caras subidos en el último cuadro de la ventana.
    pub upload_frame: u64,
    upload_mark: u64,
    free_slots: Vec<u32>,
    next_slot: u32,
    batches: Vec<Batch>,
    columns: HashMap<(u32, IVec2), Batch, crate::fxhash::FxBuild>,
    /// [render] Subidas del cuadro juntas (ver [`Subidas`]).
    subidas: std::cell::RefCell<Subidas>,
    overlay: wgpu::Buffer,
    overlay_count: u32,
    targets: Option<Targets>,
    bench_out: Option<(u32, u32, wgpu::TextureView)>,
    /// Pases de sombra (cascadas) dibujados en total (para el banco de pruebas).
    pub shadow_redraws: std::cell::Cell<u32>,
    pub adapter_name: String,
    /// [lod] Tipo de adaptador (CPU, integrada, dedicada): para recomendar la calidad.
    pub device_type: wgpu::DeviceType,
    /// Suaviza el sombreado de los escalones del terreno (evita las líneas oscuras de lejos).
    pub smooth_terrain: bool,
    /// [agua en vóxeles] Aspecto del terreno en vóxeles: de cerca, luz por cara, bordes y
    /// oclusión de cada vóxel; de lejos, igual que suave (sin curvas de nivel).
    pub voxel_terrain: bool,
    /// Nivel del río (m sobre el nivel medio): mareas, sudestadas, bajantes.
    pub water_level: f32,
    /// [entorno amplio] Hasta dónde llega el terreno dibujado: centro de los anillos de detalle
    /// (x, z) y radio (m) del círculo que cubren seguro; 0 = sin límite. Más allá, el terreno y
    /// el agua se funden con la niebla (ver `reach_fog` en voxel.wgsl y water.wgsl) y el plano
    /// del río sigue a la cámara: el borde del mundo no se ve como un corte recto.
    pub reach: [f32; 3],
    /// Luz de la luna (0 nueva, 1 llena): aclara las noches.
    pub moon: f32,
    /// Viento: de dónde viene (grados) y velocidad (m/s), para las olas.
    pub wind: (f32, f32),
    /// Tiempo en segundos (para las ondas del agua).
    pub time: f32,
}

/// Matrices del cuadro, para descartar lo que no se ve.
struct View {
    eye: Vec3,
    view_proj: Mat4,
    /// Plano cercano de la cámara (para la prueba del Hi-Z).
    near: f32,
    /// Sol de las sombras (cuantizado) y cascadas a dibujar en este cuadro: (índice, matriz,
    /// nivel de detalle máximo).
    sun: Vec3,
    cascades: Vec<(usize, Mat4, u32)>,
    /// [render] 1 / tan(fov/2) (para pasar metros a píxeles en el SSAO).
    focal: f32,
    /// [render] Dibujar el mapa de sombra de los personajes en este cuadro.
    agentes: bool,
    /// [render] Hay que volver a dibujar el mapa de sombra de los personajes (cambiaron las
    /// cajas o la matriz de la cascada 0; con la partida en pausa se reutiliza).
    agentes_redibujar: bool,
    agentes_firma: u64,
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

/// Mapas de sombra: un arreglo de texturas de profundidad, una capa por cascada.
fn make_shadow_maps(device: &wgpu::Device, q: ShadowQuality) -> (wgpu::TextureView, Vec<wgpu::TextureView>) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sombras"),
        size: wgpu::Extent3d { width: q.resolution, height: q.resolution, depth_or_array_layers: q.cascades },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let all = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
    let layers = (0..q.cascades)
        .map(|i| tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: i, array_layer_count: Some(1), ..Default::default() }))
        .collect();
    (all, layers)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
/// [render] Mapa de sombra de los personajes (2048², con la matriz de la cascada 0).
const AGENT_MAP: u32 = 2048;
fn make_agent_map(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("sombra de los personajes"),
            size: wgpu::Extent3d { width: AGENT_MAP, height: AGENT_MAP, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

#[allow(clippy::too_many_arguments)]
fn make_main_bg(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, shadow: &wgpu::TextureView, sampler: &wgpu::Sampler, origins: &wgpu::Buffer, palette: &wgpu::Buffer, sombras: &wgpu::Buffer, luces: &wgpu::Buffer, faltan: &wgpu::Buffer, fundido: &wgpu::Buffer, agentes: &wgpu::TextureView) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(shadow) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 3, resource: origins.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: palette.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: sombras.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 7, resource: luces.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 8, resource: faltan.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 9, resource: fundido.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(agentes) },
        ],
    })
}

/// Luces puntuales de los faroles (gas y sebo) para el sombreado de noche. Se arma en la CPU
/// una grilla de [`luces::LADO`]² celdas de [`luces::CELDA`] m alrededor de la cámara; cada
/// celda guarda hasta 8 luces (las más cercanas a su centro que la alcanzan). El fragmento
/// mira solo las de su celda: costo fijo y chico, sin importar cuántos faroles haya.
/// Formato (vec4): [0] = (luces, x0, z0, celda), [1] = (lado, 0, 0, 0), después 2 vec4 de
/// índices por celda (−1 = vacío) y 2 vec4 por luz: (x, y, z, alcance), (r, g, b, intensidad).
pub mod luces {
    pub const LADO: usize = 48;
    pub const CELDA: f32 = 12.0;
    pub const MAX: usize = 256;
    pub const POR_CELDA: usize = 8;
    pub const VEC4: usize = 2 + LADO * LADO * 2 + MAX * 2;

    /// Arma el contenido del búfer. `luces`: (x, y, z, alcance, intensidad); `tipo_gas` decide
    /// el color (ámbar del gas o naranja más apagado del sebo).
    pub fn armar(centro: glam::Vec2, luces: &[[f32; 5]]) -> Vec<[f32; 4]> {
        let mut out = vec![[0.0f32; 4]; VEC4];
        let n = luces.len().min(MAX);
        let x0 = (centro.x / CELDA).floor() * CELDA - CELDA * (LADO / 2) as f32;
        let z0 = (centro.y / CELDA).floor() * CELDA - CELDA * (LADO / 2) as f32;
        out[0] = [n as f32, x0, z0, CELDA];
        out[1] = [LADO as f32, 0.0, 0.0, 0.0];
        for c in 0..LADO * LADO {
            out[2 + c * 2] = [-1.0; 4];
            out[3 + c * 2] = [-1.0; 4];
        }
        let base = 2 + LADO * LADO * 2;
        let mut celdas: Vec<Vec<(f32, usize)>> = vec![Vec::new(); LADO * LADO];
        for (i, l) in luces.iter().take(n).enumerate() {
            let gas = l[4] > 0.6;
            // Ámbar del gas (#F2B862 en lineal) o el naranja de la vela de sebo.
            let col = if gas { [0.89, 0.48, 0.12] } else { [1.0, 0.45, 0.12] };
            out[base + i * 2] = [l[0], l[1], l[2], l[3]];
            out[base + i * 2 + 1] = [col[0], col[1], col[2], l[4]];
            let r = l[3];
            let (cx0, cx1) = (((l[0] - r - x0) / CELDA).floor() as i32, ((l[0] + r - x0) / CELDA).floor() as i32);
            let (cz0, cz1) = (((l[2] - r - z0) / CELDA).floor() as i32, ((l[2] + r - z0) / CELDA).floor() as i32);
            for cz in cz0.max(0)..=cz1.min(LADO as i32 - 1) {
                for cx in cx0.max(0)..=cx1.min(LADO as i32 - 1) {
                    let cc = glam::Vec2::new(x0 + (cx as f32 + 0.5) * CELDA, z0 + (cz as f32 + 0.5) * CELDA);
                    let d = cc.distance(glam::Vec2::new(l[0], l[2]));
                    celdas[cz as usize * LADO + cx as usize].push((d, i));
                }
            }
        }
        for (c, v) in celdas.iter_mut().enumerate() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (k, &(_, i)) in v.iter().take(POR_CELDA).enumerate() {
                out[2 + c * 2 + k / 4][k % 4] = i as f32;
            }
        }
        out
    }
}

fn make_cull_bg(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, cols: &wgpu::Buffer, args: &wgpu::Buffer, params: &wgpu::Buffer, visible: &wgpu::Buffer, hiz: &wgpu::TextureView) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: cols.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: args.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: visible.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(hiz) },
        ],
    })
}

/// Suma de argumentos indirectos (6 por columna): (columnas con algo, caras).
fn sum_args(args: &[DrawArgs]) -> (u32, u64) {
    let mut cols = 0;
    let mut quads = 0u64;
    for c in args.chunks(6) {
        let n: u64 = c.iter().map(|a| a.instance_count as u64).sum();
        if n > 0 {
            cols += 1;
        }
        quads += n;
    }
    (cols, quads)
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
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                storage_entry(7, wgpu::ShaderStages::FRAGMENT, true),
                // [render] Niebla de lo que falta (ver `Renderer::set_faltan`).
                storage_entry(8, wgpu::ShaderStages::FRAGMENT, true),
                // [render] Fundido entre niveles: avance por columna (ver `Renderer::fundidos`).
                storage_entry(9, wgpu::ShaderStages::FRAGMENT, true),
                // [render] Sombra de los personajes y animales (ver `Renderer::agent_map`).
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        // Pase de sombras: orígenes de las columnas y la matriz de la cascada (desplazamiento
        // dinámico de 256 bytes por cascada).
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sombras"),
            entries: &[
                storage_entry(3, wgpu::ShaderStages::VERTEX, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(64) },
                    count: None,
                },
            ],
        });
        let sombras = device.create_buffer(&wgpu::BufferDescriptor { label: Some("cascadas"), size: std::mem::size_of::<SombrasGpu>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let cascade_vp = device.create_buffer(&wgpu::BufferDescriptor { label: Some("matrices de cascada"), size: 256 * MAX_CASCADES as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let quality = ShadowQuality::DEFAULT;
        let (shadow_view, shadow_layers) = make_shadow_maps(&device, quality);
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
        let luces_ini = luces::armar(glam::Vec2::ZERO, &[]);
        let luces_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("luces de los faroles"), contents: bytemuck::cast_slice(&luces_ini), usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST });
        let faltan_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("niebla de lo que falta"), contents: bytemuck::cast_slice(&[crate::stream::SIN_FALTA; crate::stream::SECTORES]), usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST });
        let fundido_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fundido entre niveles"), size: ORIGIN_SLOTS as u64 * 4, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let agent_map = make_agent_map(&device);
        let main_bg = make_main_bg(&device, &main_bgl, &globals, &shadow_view, &shadow_sampler, &origins, &palette, &sombras, &luces_buf, &faltan_buf, &fundido_buf, &agent_map);
        // [render] Matriz de la cascada 0 para el mapa de sombra de los personajes.
        let agent_vp = device.create_buffer(&wgpu::BufferDescriptor { label: Some("matriz de la sombra de los personajes"), size: 256, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let shadow_vox_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 3, resource: origins.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &cascade_vp, offset: 0, size: wgpu::BufferSize::new(64) }) },
            ],
        });
        let agent_cast_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 3, resource: origins.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &agent_vp, offset: 0, size: wgpu::BufferSize::new(64) }) },
            ],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &simple_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let shader = |label, src: &str| device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let with_shadows = |src: &str| format!("{}\n{}", include_str!("shaders/sombras.wgsl"), src);
        let voxel_mod = shader("voxel", &with_shadows(include_str!("shaders/voxel.wgsl")));
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
        let quad_layout_agua = quad_layout.clone();
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
        // [render] Fundido entre niveles: mismo vértice, fragmento con tramado (`discard`).
        let fade_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxeles (fundido)"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState { module: &voxel_mod, entry_point: "vs_main", compilation_options: Default::default(), buffers: &[quad_layout.clone()] },
            fragment: Some(wgpu::FragmentState { module: &voxel_mod, entry_point: "fs_fade", compilation_options: Default::default(), targets: &[Some(format.into())] }),
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
                // Posición y corriente (m/s en x, z; cero en el río).
                buffers: &[wgpu::VertexBufferLayout { array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2] }],
            },
            fragment: Some(wgpu::FragmentState { module: &water_mod, entry_point: "fs_plano", compilation_options: Default::default(), targets: &[blend_target.clone()] }),
            primitive: Default::default(),
            // Sin sesgo de profundidad: la tierra firme nunca queda al ras del nivel del mar (ver
            // `stream::top_from_height`: su cara de arriba está por lo menos un vóxel arriba, y la
            // del fondo, por debajo), así que no hay empate que resolver. Un sesgo por pendiente
            // acercaba los planos grandes del agua, vistos de costado, varios metros a la cámara
            // y el agua se dibujaba encima de la tierra baja.
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        // [agua en vóxeles] Caras del agua: una instancia por cara, como el terreno, con mezcla.
        let water_quad_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agua en vóxeles"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState { module: &water_mod, entry_point: "vs_quad", compilation_options: Default::default(), buffers: &[quad_layout_agua] },
            fragment: Some(wgpu::FragmentState { module: &water_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[blend_target.clone()] }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let detail_mod = shader("flora", &with_shadows(include_str!("shaders/detail.wgsl")));
        let detail_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flora"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState {
                module: &detail_mod,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                // Una instancia por caja (trees::Micro, 20 bytes): esquina, tamaño, material.
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::trees::Micro>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint8x4, 2 => Uint8x4],
                }],
            },
            fragment: Some(wgpu::FragmentState { module: &detail_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(reverse_depth(true, wgpu::CompareFunction::Greater)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        let agent_mod = shader("agentes", &with_shadows(include_str!("shaders/agentes.wgsl")));
        // [personajes]
        let agent_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::detalle::Caja>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32, 4 => Float32x3, 5 => Unorm8x4],
        };
        // Sombras de contacto bajo los modelos: las mismas instancias (las de alto 0), mezcla
        // que oscurece lo que ya está dibujado, sin escribir profundidad.
        let agent_shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agentes-sombra"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState { module: &agent_mod, entry_point: "vs_sombra", compilation_options: Default::default(), buffers: &[agent_layout.clone()] },
            fragment: Some(wgpu::FragmentState {
                module: &agent_mod,
                entry_point: "fs_sombra",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(reverse_depth(false, wgpu::CompareFunction::GreaterEqual)),
            multisample: ms,
            multiview: None,
            cache: None,
        });
        // [/personajes]
        // [render] Personajes y animales en el mapa de sombra propio (proyectan sombra en las
        // cascadas): mismas instancias, solo profundidad, con el sesgo del pase de sombras.
        let agent_cast_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agentes: sombra proyectada"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &agent_mod, entry_point: "vs_proyecta", compilation_options: Default::default(), buffers: &[agent_layout.clone()] },
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
        let agent_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agentes"),
            layout: Some(&main_layout),
            vertex: wgpu::VertexState {
                module: &agent_mod,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                // [personajes] Una instancia por caja (48 bytes, `detalle::Caja`): pivote, giro,
                // desde el pivote, inclinación, tamaño y color.
                buffers: &[agent_layout.clone()],
            },
            fragment: Some(wgpu::FragmentState { module: &agent_mod, entry_point: "fs_main", compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(reverse_depth(true, wgpu::CompareFunction::Greater)),
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
        let mut wv: Vec<[f32; 5]> = Vec::new();
        // 400 km en baldosas de 2 km: la curvatura de la Tierra se aplica en cada vértice, y con
        // baldosas más grandes el centro de cada una quedaba metros por debajo del nivel real.
        let (c, s, n) = (12000.0f32, 2000.0f32, 200);
        for j in 0..n {
            for i in 0..n {
                let x0 = c - s * n as f32 / 2.0 + i as f32 * s;
                let z0 = c - s * n as f32 / 2.0 + j as f32 * s;
                let (x1, z1) = (x0 + s, z0 + s);
                wv.extend_from_slice(&[[x0, 0.0, z0, 0.0, 0.0], [x0, 0.0, z1, 0.0, 0.0], [x1, 0.0, z1, 0.0, 0.0], [x0, 0.0, z0, 0.0, 0.0], [x1, 0.0, z1, 0.0, 0.0], [x1, 0.0, z0, 0.0, 0.0]]);
            }
        }
        let water = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("agua"), contents: bytemuck::cast_slice(&wv), usage: wgpu::BufferUsages::VERTEX });
        let alloc = Buddy::new(INITIAL_ORDER);
        let quad_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("índices de cara"), contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3, 0, 0]), usage: wgpu::BufferUsages::INDEX });
        let quads = make_quad_buffer(&device, alloc.capacity());
        let buf = |label: &str, size: u64, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let cols = buf("columnas", ORIGIN_SLOTS as u64 * 64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let args_size = ORIGIN_SLOTS as u64 * 6 * std::mem::size_of::<DrawArgs>() as u64;
        let indirect_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_SRC;
        let args_main = buf("args principal", args_size, indirect_usage);
        let args_main2 = buf("args principal (Hi-Z)", args_size, indirect_usage);
        let args_shadow: Vec<wgpu::Buffer> = (0..MAX_CASCADES).map(|_| buf("args sombras", args_size, indirect_usage)).collect();
        let args_cpu = buf("args cpu", args_size * 2, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST);
        let args_agua = buf("args agua", ORIGIN_SLOTS as u64 * std::mem::size_of::<DrawArgs>() as u64, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST);
        let params_size = std::mem::size_of::<CullParams>() as u64;
        let params_usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let cull_params_main = buf("descarte principal", params_size, params_usage);
        let cull_params_main2 = buf("descarte principal (Hi-Z)", params_size, params_usage);
        let cull_params_shadow: Vec<wgpu::Buffer> = (0..MAX_CASCADES).map(|_| buf("descarte sombras", params_size, params_usage)).collect();
        let visible = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("visibles"), contents: bytemuck::cast_slice(&vec![1u32; ORIGIN_SLOTS as usize]), usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST });
        let float_tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
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
                storage_entry(3, wgpu::ShaderStages::COMPUTE, false),
                float_tex(4),
            ],
        });
        // Pirámide vacía para los descartes que no la leen (el shader declara la textura).
        let dummy_hiz = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("hi-z vacío"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let cull_bg_main = make_cull_bg(&device, &cull_bgl, &cols, &args_main, &cull_params_main, &visible, &dummy_hiz);
        let cull_bg_shadow: Vec<wgpu::BindGroup> = (0..MAX_CASCADES).map(|i| make_cull_bg(&device, &cull_bgl, &cols, &args_shadow[i], &cull_params_shadow[i], &visible, &dummy_hiz)).collect();
        let cull_pipe = if can_indirect {
            let module = shader("descarte", include_str!("shaders/cull.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&cull_bgl], push_constant_ranges: &[] });
            Some(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("descarte"), layout: Some(&layout), module: &module, entry_point: "main", compilation_options: Default::default(), cache: None }))
        } else {
            None
        };
        // Hi-Z: hace falta el descarte en GPU, compute y escribir texturas R32Float. Si falta
        // algo, se queda el descarte por volumen de visión solo.
        let hiz_supported = cull_pipe.is_some()
            && adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
            && adapter.get_texture_format_features(wgpu::TextureFormat::R32Float).allowed_usages.contains(wgpu::TextureUsages::STORAGE_BINDING);
        let hiz_pipes = hiz_supported.then(|| {
            let src = include_str!("shaders/hiz.wgsl")
                .replace("SRC_TYPE", if msaa > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" })
                .replace("SAMPLES", if msaa > 1 { "textureNumSamples(src)" } else { "1u" });
            let module = shader("hi-z", &src);
            let storage = wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::R32Float, view_dimension: wgpu::TextureViewDimension::D2 },
                count: None,
            };
            let reduce_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("hi-z nivel 0"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: msaa > 1 },
                        count: None,
                    },
                    storage,
                ],
            });
            let down_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("hi-z niveles"), entries: &[float_tex(2), storage] });
            let pipe = |bgl: &wgpu::BindGroupLayout, entry| {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[bgl], push_constant_ranges: &[] });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("hi-z"), layout: Some(&layout), module: &module, entry_point: entry, compilation_options: Default::default(), cache: None })
            };
            HizPipes { reduce: pipe(&reduce_bgl, "reduce"), down: pipe(&down_bgl, "down"), reduce_bgl, down_bgl }
        });
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
        let hiz = match HIZ_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => false,
            2 => hiz_supported,
            _ => hiz_supported && draw_mode == DrawMode::GpuCull,
        };
        let overlay = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resaltado"),
            size: (std::mem::size_of::<OverlayVertex>() * OVERLAY_MAX) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let post = PostPipes::new(&device, format, msaa);
        Ok(Renderer {
            device,
            queue,
            format,
            msaa,
            globals,
            main_bg,
            main_bgl,
            shadow_bg,
            shadow_sampler,
            palette,
            luces: luces_buf,
            luces_key: 0,
            faltan: faltan_buf,
            fundido: fundido_buf,
            entrantes: Default::default(),
            salientes: Default::default(),
            reloj: std::time::Instant::now(),
            reloj_fijo: None,
            fade_pipe,
            post,
            fundir: false,
            ssao: false,
            escala: 100,
            faltan_val: [crate::stream::SIN_FALTA; crate::stream::SECTORES],
            shadow_view,
            shadow_layers,
            quality,
            sombras,
            cascade_vp,
            casc: std::cell::RefCell::new(CascState::default()),
            shadow_budget: usize::MAX,
            voxel_pipe,
            shadow_pipe,
            sky_pipe,
            water_pipe,
            water_quad_pipe,
            overlay_pipe,
            detail_pipe,
            detail_cols: HashMap::default(),
            detail_static: Vec::new(),
            detail_buf: std::cell::RefCell::new(None),
            detail_dirty: std::cell::Cell::new(false),
            agent_pipe,
            agent_shadow_pipe, // [personajes]
            agent_map,
            agent_vp,
            agent_cast_bg,
            agent_cast_pipe,
            sombra_agentes: true,
            agent_shadow_key: Default::default(),
            agent_buf: None,
            agent_count: 0,
            agent_key: 0,
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
            args_agua,
            cull_pipe,
            cull_params_main,
            cull_params_shadow,
            cull_bg_main,
            cull_bg_shadow,
            hiz,
            hiz_supported,
            hiz_pipes,
            visible,
            cull_bgl,
            args_main2,
            cull_params_main2,
            draw_mode,
            can_indirect,
            timestamps,
            gpu_read: Default::default(),
            last_gpu_ms: None,
            last_wait_ms: 0.0,
            destinos_rehechos: 0,
            uploaded_bytes: 0,
            scene_version: 0,
            overlay_key: 0,
            last_draws: (0, 0),
            last_shadow_passes: 0,
            upload_frame: 0,
            upload_mark: 0,
            free_slots: Vec::new(),
            next_slot: 0,
            batches: Vec::new(),
            columns: HashMap::default(),
            subidas: std::cell::RefCell::new(Subidas::new()),
            overlay,
            overlay_count: 0,
            targets: None,
            bench_out: None,
            shadow_redraws: std::cell::Cell::new(0),
            adapter_name,
            device_type: adapter.get_info().device_type,
            smooth_terrain: true,
            voxel_terrain: true,
            water_level: 0.0,
            reach: [0.0; 3],
            moon: 0.5,
            wind: (70.0, 4.0),
            time: 3.0,
        })
    }

    /// Sube mallas sueltas a la GPU (modo captura: cada malla con su origen).
    pub fn upload(&mut self, meshes: &[(u32, Mesh)]) {
        for (_, m) in meshes {
            if !m.detail.is_empty() {
                self.detail_static.extend_from_slice(&m.detail);
                self.detail_dirty.set(true);
            }
        }
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
        self.subidas.get_mut().poner_col(b.slot, ColGpu::zeroed(), None, false);
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
            // [render] Lo que ya se copió al búfer viejo tiene que llegar antes de la copia.
            self.flush_uploads();
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
        if m.is_empty() {
            return None;
        }
        let (lo, hi) = m.bounds();
        self.shadow_dirty_box(lo, hi);
        let n = m.quad_count();
        let mut groups = [0u32; 7];
        let mut acc = 0u32;
        for (k, g) in m.quads.iter().enumerate() {
            groups[k] = acc;
            acc += g.len() as u32;
        }
        groups[6] = acc;
        let slot = self.alloc_slot();
        let (start, order) = self.alloc_quads(n as u32);
        // [render] Las caras se copian directo al búfer intermedio del cuadro (sin un Vec de
        // por medio), con el número de columna puesto.
        let bytes = n as u64 * QUAD_BYTES;
        {
            let sub = self.subidas.get_mut();
            let mut view = sub.escribir(&self.device, &self.quads, start as u64 * QUAD_BYTES, bytes);
            let dst: &mut [Quad] = bytemuck::cast_slice_mut(&mut view[..]);
            let mut i = 0;
            for src in m.quads.iter().chain(std::iter::once(&m.water)) {
                for q in src {
                    dst[i] = Quad { slot, ..*q };
                    i += 1;
                }
            }
        }
        self.uploaded_bytes += bytes;
        let origin = [m.origin.x, m.origin.y, m.origin.z, m.voxel];
        let b = Batch { start, order, groups, quads: n as u32, water: m.water.len() as u32, lod, entra: false, slot, lo, hi };
        // Columna nueva: se dibuja en la primera pasada del Hi-Z (seguramente tapa algo).
        self.subidas.get_mut().poner_col(slot, col_gpu(&b, ESTABLE), Some(origin), true);
        Some(b)
    }

    /// Un cambio de geometría solo obliga a rehacer una cascada si cae dentro de su caja de luz
    /// (clave de sombra por cascada, técnica de Tisma engine).
    fn shadow_dirty_box(&self, lo: Vec3, hi: Vec3) {
        let mut st = self.casc.borrow_mut();
        for i in 0..MAX_CASCADES {
            match st.planes[i] {
                Some(p) if !box_visible(&p, lo, hi) => {}
                _ => st.dirty[i] = true,
            }
        }
    }

    /// Reemplaza la malla de una columna del mundo en streaming (vacía = la quita).
    pub fn set_column(&mut self, key: (u32, IVec2), mesh: &Mesh) {
        self.set_detail(key, &mesh.detail);
        let new = self.make_batch(mesh, key.0);
        let old = match new {
            Some(b) => self.columns.insert(key, b),
            None => self.columns.remove(&key),
        };
        if let Some(b) = old {
            self.shadow_dirty_box(b.lo, b.hi);
            self.free_batch(b);
        }
    }

    fn set_detail(&mut self, key: (u32, IVec2), d: &[crate::trees::Micro]) {
        if d.is_empty() {
            if self.detail_cols.remove(&key).is_some() {
                self.detail_dirty.set(true);
            }
        } else {
            self.detail_cols.insert(key, d.to_vec());
            self.detail_dirty.set(true);
        }
    }

    /// Rearma el búfer de la flora si cambió alguna columna (a lo sumo una vez por cuadro).
    fn refresh_detail(&self) {
        if !self.detail_dirty.replace(false) {
            return;
        }
        let n: usize = self.detail_static.len() + self.detail_cols.values().map(|v| v.len()).sum::<usize>();
        if n == 0 {
            *self.detail_buf.borrow_mut() = None;
            return;
        }
        let mut all = Vec::with_capacity(n);
        all.extend_from_slice(&self.detail_static);
        for v in self.detail_cols.values() {
            all.extend_from_slice(v);
        }
        let buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("flora"), contents: bytemuck::cast_slice(&all), usage: wgpu::BufferUsages::VERTEX });
        *self.detail_buf.borrow_mut() = Some((buf, n as u32));
    }

    /// [render] Niebla de lo que falta (técnicas del mod LOD §2.12, `NieblaSectores`): por cada
    /// uno de los sectores alrededor del centro de los anillos, la distancia a la primera zona
    /// que todavía no tiene nada dibujado. Los shaders funden con la niebla desde un poco antes:
    /// al entrar, teletransportarse o alejar rápido, lo que falta no se ve como el plano del río.
    /// Se sube solo si cambió.
    pub fn set_faltan(&mut self, d: &[f32; crate::stream::SECTORES]) {
        if *d == self.faltan_val {
            return;
        }
        self.faltan_val = *d;
        self.scene_version += 1;
        self.queue.write_buffer(&self.faltan, 0, bytemuck::cast_slice(d));
    }

    /// Luces de los faroles alrededor de `centro` (x, y, z, alcance, intensidad): se suben solo
    /// si cambiaron.
    pub fn set_lights(&mut self, centro: glam::Vec2, ls: &[[f32; 5]]) {
        let celda = (centro / luces::CELDA).floor();
        let mut h = crate::fxhash::FxHasher::default();
        std::hash::Hasher::write(&mut h, bytemuck::cast_slice(&[celda.x, celda.y]));
        for l in ls {
            std::hash::Hasher::write(&mut h, bytemuck::cast_slice(l));
        }
        let key = std::hash::Hasher::finish(&h);
        if key == self.luces_key {
            return;
        }
        self.luces_key = key;
        self.scene_version += 1;
        let datos = luces::armar(centro, ls);
        self.queue.write_buffer(&self.luces, 0, bytemuck::cast_slice(&datos));
    }

    /// Cajas de los vecinos y vehículos de este cuadro (vacío: no se dibuja ninguno).
    pub fn set_agents(&mut self, cajas: &[crate::detalle::Caja]) {
        let bytes: &[u8] = bytemuck::cast_slice(cajas);
        let mut h = crate::fxhash::FxHasher::default();
        std::hash::Hasher::write(&mut h, bytes);
        let key = std::hash::Hasher::finish(&h);
        if key == self.agent_key && cajas.len() as u32 == self.agent_count {
            return;
        }
        self.agent_key = key;
        self.agent_count = cajas.len() as u32;
        self.scene_version += 1;
        if cajas.is_empty() {
            return;
        }
        if self.agent_buf.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            let size = (bytes.len() as u64).next_power_of_two().max(64 * 1024);
            self.agent_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("agentes"), size, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }));
        }
        self.queue.write_buffer(self.agent_buf.as_ref().unwrap(), 0, bytes);
    }

    /// [render] El mapa de sombra de los personajes se vuelve a dibujar en el próximo cuadro
    /// (aunque no hayan cambiado las cajas; para el banco).
    pub fn rehacer_sombra_agentes(&self) {
        self.agent_shadow_key.set(0);
    }

    /// Cajas de agentes del último cuadro.
    pub fn agent_count(&self) -> u32 {
        self.agent_count
    }

    /// Piezas de flora baja cargadas.
    pub fn detail_count(&self) -> usize {
        self.detail_static.len() + self.detail_cols.values().map(|v| v.len()).sum::<usize>()
    }

    pub fn remove_column(&mut self, key: (u32, IVec2)) {
        self.set_detail(key, &[]);
        if let Some(b) = self.columns.remove(&key) {
            self.shadow_dirty_box(b.lo, b.hi);
            self.free_batch(b);
        }
    }

    /// [render] Efectos de las opciones (fundido, SSAO, escala de render).
    pub fn aplicar_efectos(&mut self, s: &crate::save::Settings) {
        self.fundir = s.fundido;
        if !self.fundir {
            self.terminar_fundidos();
        }
        if self.ssao != s.ssao || self.escala != s.escala_render {
            self.scene_version += 1;
        }
        self.ssao = s.ssao;
        self.escala = escala::valida(s.escala_render);
    }

    /// [render] Columna que reemplaza a otra de otro nivel (técnicas del mod LOD §2.5): entra
    /// con tramado en [`FUNDIDO_S`] mientras la vieja sigue entera; hasta que termina no va en
    /// la pasada principal ni en el Hi-Z (tiene huecos), sí en las sombras.
    pub fn set_column_fade(&mut self, key: (u32, IVec2), mesh: &Mesh) {
        self.set_column(key, mesh);
        if !self.fundir {
            return;
        }
        let t = self.reloj.elapsed().as_secs_f32();
        if let Some(b) = self.columns.get_mut(&key) {
            b.entra = true;
            let (slot, col) = (b.slot, col_gpu(b, ENTRA));
            self.subidas.get_mut().poner_col(slot, col, None, false);
            self.subidas.get_mut().poner_fundido(slot, t + 1.0);
            self.entrantes.push_back((key, slot, t));
        }
    }

    /// [render] Columna que se va porque otras ya la cubren: se desvanece con el tramado
    /// complementario en [`FUNDIDO_S`] y recién después se libera. No tira sombra.
    pub fn fade_out_column(&mut self, key: (u32, IVec2)) {
        if !self.fundir {
            return self.remove_column(key);
        }
        self.set_detail(key, &[]);
        if let Some(mut b) = self.columns.remove(&key) {
            self.scene_version += 1;
            self.shadow_dirty_box(b.lo, b.hi);
            b.entra = false;
            let t = self.reloj.elapsed().as_secs_f32();
            self.subidas.get_mut().poner_col(b.slot, col_gpu(&b, SALE), None, false);
            self.subidas.get_mut().poner_fundido(b.slot, -(t + 1.0));
            self.salientes.push_back((b, t));
        }
    }

    /// [render] Termina los fundidos vencidos (las que entran pasan a estables; las que salen se
    /// liberan). El avance no se escribe por cuadro: el shader lo saca del reloj de las globales
    /// (`cam_pos.w`) y del momento en que empezó cada columna (`fundido[slot]`, escrito una sola
    /// vez). Escribirlo en cada cuadro eran ~280 escrituras por cuadro en el vuelo (p50 0,06 →
    /// 0,95 ms).
    fn avanzar_fundidos(&mut self, terminar: bool) {
        if self.entrantes.is_empty() && self.salientes.is_empty() {
            return;
        }
        self.scene_version += 1;
        let now = self.reloj.elapsed().as_secs_f32();
        let avance = |t0: f32| if terminar { 1.0 } else { ((now - t0) / FUNDIDO_S).clamp(0.0, 1.0) };
        // [render] Las colas están en orden de comienzo: solo se miran las del frente que
        // vencieron (antes se recorrían las ~280 en cada cuadro del vuelo). Una entrada cuya
        // columna se reemplazó o quitó (otro lugar, o ya no entra) se descarta.
        while let Some(&(key, slot, t0)) = self.entrantes.front() {
            if avance(t0) < 1.0 {
                break;
            }
            self.entrantes.pop_front();
            if let Some(b) = self.columns.get_mut(&key).filter(|b| b.slot == slot && b.entra) {
                b.entra = false;
                let col = col_gpu(b, ESTABLE);
                self.subidas.get_mut().poner_col(slot, col, None, true);
            }
        }
        while let Some((_, t0)) = self.salientes.front() {
            if avance(*t0) < 1.0 {
                break;
            }
            let (b, _) = self.salientes.pop_front().unwrap();
            self.free_batch(b);
        }
    }

    /// [render] Termina ya todos los fundidos (antes de una captura o al medir quieto).
    pub fn terminar_fundidos(&mut self) {
        self.reloj_fijo = None;
        self.avanzar_fundidos(true);
    }

    /// [render] Columnas fundiéndose (entran, salen).
    pub fn fundidos(&self) -> (usize, usize) {
        (self.entrantes.len(), self.salientes.len())
    }

    /// [render] Banco: pone a entrar con fundido las `n` columnas más cercanas a `eye` (el punto
    /// mirado), para medir el costo de la pasada del fundido en el peor caso, sin vencerse solas.
    pub fn forzar_fundido(&mut self, eye: Vec3, n: usize) {
        let mut keys: Vec<((u32, IVec2), f32)> = self.columns.iter().map(|(k, b)| (*k, ((b.lo + b.hi) * 0.5 - eye).length_squared())).collect();
        keys.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0 .0.cmp(&b.0 .0)).then(a.0 .1.x.cmp(&b.0 .1.x)).then(a.0 .1.y.cmp(&b.0 .1.y)));
        // Reloj quieto a mitad del fundido.
        let t = self.reloj.elapsed().as_secs_f32();
        self.reloj_fijo = Some(t + FUNDIDO_S * 0.5);
        for (key, _) in keys.into_iter().take(n) {
            let b = self.columns.get_mut(&key).unwrap();
            b.entra = true;
            let (slot, col) = (b.slot, col_gpu(b, ENTRA));
            self.subidas.get_mut().poner_col(slot, col, None, false);
            self.subidas.get_mut().poner_fundido(slot, t + 1.0);
            // Sin vencimiento: el reloj de inicio en el futuro lejano (del lado de la CPU).
            self.entrantes.push_back((key, slot, f32::MAX / 4.0));
        }
    }

    /// [render] Envía las subidas juntadas en el cuadro (antes de dibujar o de agrandar el
    /// megabúfer).
    fn flush_uploads(&self) -> bool {
        self.subidas.borrow_mut().enviar(&self.device, &self.queue, &self.cols, &self.origins, &self.visible, &self.fundido)
    }

    /// Avanza el contador de cuadros y libera los bloques que la GPU ya no usa.
    pub fn maintain(&mut self) {
        // Un envío (aunque esté vacío) hace que wgpu ejecute las copias pendientes de las
        // mallas nuevas y libere sus búferes intermedios; sin ventana casi no hay envíos.
        if !self.flush_uploads() {
            self.queue.submit(std::iter::empty());
        }
        self.device.poll(wgpu::Maintain::Poll);
        self.frame += 1;
        self.alloc.collect(self.frame, FRAMES_IN_FLIGHT);
        self.avanzar_fundidos(false);
    }

    /// Memoria de las mallas en la GPU (bytes de caras realmente usados), en MB.
    pub fn mesh_mb(&self) -> f64 {
        self.quad_count() as f64 * QUAD_BYTES as f64 / 1048576.0
    }

    /// Tamaño del megabúfer de caras (reservado), en MB.
    pub fn megabuffer_mb(&self) -> f64 {
        self.quads.size() as f64 / 1048576.0
    }

    /// Obliga a rehacer todas las cascadas de sombra en el próximo cuadro (sin límite).
    pub fn invalidate_shadows(&self) {
        let mut st = self.casc.borrow_mut();
        st.dirty = [true; MAX_CASCADES];
        st.force = true;
    }

    /// Calidad de las sombras (opciones gráficas): rehace los mapas y vuelve a dibujarlos.
    pub fn set_shadow_quality(&mut self, q: ShadowQuality) {
        let q = q.sanitized();
        if q == self.quality {
            return;
        }
        self.quality = q;
        let (all, layers) = make_shadow_maps(&self.device, q);
        self.main_bg = make_main_bg(&self.device, &self.main_bgl, &self.globals, &all, &self.shadow_sampler, &self.origins, &self.palette, &self.sombras, &self.luces, &self.faltan, &self.fundido, &self.agent_map);
        self.shadow_view = all;
        self.shadow_layers = layers;
        *self.casc.borrow_mut() = CascState { force: true, dirty: [true; MAX_CASCADES], ..Default::default() };
        self.scene_version += 1;
    }

    pub fn shadow_quality(&self) -> ShadowQuality {
        self.quality
    }

    /// Quita todas las mallas (banco de pruebas).
    pub fn clear_columns(&mut self) {
        self.invalidate_shadows();
        self.entrantes.clear();
        self.salientes.clear();
        self.columns.clear();
        self.detail_cols.clear();
        self.detail_static.clear();
        self.detail_dirty.set(true);
        self.batches.clear();
        self.free_slots.clear();
        self.next_slot = 0;
        // [render] Lo pendiente va antes de borrar la tabla (si no, la pisaría después).
        self.flush_uploads();
        self.subidas.get_mut().cols.fill(ColGpu::zeroed());
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
        self.flush_uploads(); // [render]
        if !self.targets_ok(self.targets.as_ref(), width, height) {
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
        // [lod] Si la ventana dejó una lectura de tiempos en vuelo, se termina antes de reusar el búfer.
        if self.gpu_read.load(std::sync::atomic::Ordering::SeqCst) != 0 {
            self.device.poll(wgpu::Maintain::Wait);
            if let Some((_, _, read, _)) = self.timestamps.as_ref().filter(|_| self.gpu_read.load(std::sync::atomic::Ordering::SeqCst) == 2) {
                read.unmap();
            }
            self.gpu_read.store(0, std::sync::atomic::Ordering::SeqCst);
        }
        let t0 = std::time::Instant::now();
        let view = self.write_globals(cam, hour, width as f32 / height as f32, self.shadow_budget);
        let mut enc = self.device.create_command_encoder(&Default::default());
        let ts = self.timestamps.as_ref().map(|t| &t.0);
        let mut st = self.encode_frame(&mut enc, self.targets.as_ref().unwrap(), &self.bench_out.as_ref().unwrap().2, &view, ts);
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
        // En modo GPU, lo dibujado se cuenta leyendo los argumentos indirectos.
        if self.draw_mode == DrawMode::GpuCull && self.cull_pipe.is_some() {
            let n = self.next_slot;
            let (c1, q1) = self.read_args(&self.args_main, n);
            let (c2, q2) = if st.hiz { self.read_args(&self.args_main2, n) } else { (0, 0) };
            st.cols_main = c1 + c2;
            st.quads_main = q1 + q2;
            st.quads_phase2 = q2;
            st.quads_shadow = view.cascades.iter().map(|(i, _, _)| self.read_args(&self.args_shadow[*i], n).1).sum();
        }
        self.frame += 1;
        self.alloc.collect(self.frame, FRAMES_IN_FLIGHT);
        FrameStats { cpu_ms: cpu.as_secs_f64() * 1000.0, total_ms, gpu_ms, ..st }
    }

    /// Lee un búfer de argumentos indirectos (banco de pruebas): (columnas, caras).
    fn read_args(&self, buf: &wgpu::Buffer, slots: u32) -> (u32, u64) {
        let size = slots as u64 * 6 * std::mem::size_of::<DrawArgs>() as u64;
        if size == 0 {
            return (0, 0);
        }
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buf, 0, &staging, 0, size);
        self.queue.submit([enc.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);
        let r = sum_args(bytemuck::cast_slice(&slice.get_mapped_range()));
        staging.unmap();
        r
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
        // Con cascadas de sombra esperando, el cuadro siguiente no es igual aunque nada cambie.
        let pending = if self.casc.borrow().pending { self.frame } else { 0 };
        [std::hash::Hasher::finish(&h), self.scene_version + (pending << 32), size.0 as u64, size.1 as u64]
    }

    /// Franja de alturas donde puede haber algo que reciba sombra: del río (−6 m) al techo más
    /// alto de lo cargado en los niveles que proyectan sombra.
    fn shadow_band(&self) -> (f32, f32) {
        let mut ymax = f32::NEG_INFINITY;
        for b in self.batches.iter().chain(self.columns.values()) {
            if b.lod <= SHADOW_FAR_LOD {
                ymax = ymax.max(b.hi.y);
            }
        }
        if !ymax.is_finite() {
            ymax = 60.0;
        }
        (-6.0, ymax.max(10.0) + 2.0)
    }

    /// Ubica las cascadas para esta cámara y decide cuáles se vuelven a dibujar: las que
    /// cambiaron de lugar o de sol, o cuya geometría cambió; a lo sumo `budget` por cuadro
    /// (salvo que se haya pedido rehacer todo). Las que esperan se siguen usando con la matriz
    /// con la que se dibujaron, así lo que se lee es coherente.
    fn plan_cascades(&self, cam: &OrbitCamera, aspect: f32, sun: Vec3, budget: usize) -> (SombrasGpu, Vec<(usize, Mat4, u32)>) {
        let q = self.quality;
        let n = q.cascades as usize;
        let res = q.resolution;
        let rot = cascades::light_rotation(sun);
        let (ymin, ymax) = self.shadow_band();
        let eye = cam.eye();
        let frame = ViewFrame::new(eye, cam.target, cam.fov_y, aspect);
        let half = cam.fov_y.to_radians() * 0.5;
        let pitch = cam.pitch.to_radians();
        let near = (cam.distance * 0.002).clamp(0.1, 20.0);
        let a_bot = (pitch + half).min(89f32.to_radians());
        let a_top = pitch - half;
        // La primera cascada empieza donde puede empezar a verse algo (debajo del techo más
        // alto) y termina un poco más allá del objetivo de la cámara; la última, donde se deja
        // de ver el suelo o a la distancia de sombra (8 veces la de la cámara, 1,5 a 8 km, y
        // siempre más allá del objetivo).
        let n0 = if eye.y > ymax { ((eye.y - ymax) / a_bot.sin() * half.cos() * 0.9).max(near) } else { near };
        let cap = (cam.distance * 8.0).clamp(1500.0, 8000.0).max(cam.distance * 2.5);
        let far = if a_top > 0.03 { ((eye.y - ymin) / a_top.sin() * half.cos()).min(cap) } else { cap };
        let sp = cascades::splits(n0, cam.distance * 1.2, far, n);
        // Hacia el sol, la caja llega hasta donde puede haber algo que dé sombra.
        let reach = ((ymax - ymin) / sun.y.max(0.05)).min(6000.0) + 10.0;
        let mut st = self.casc.borrow_mut();
        if st.sun != sun {
            st.want = [None; MAX_CASCADES];
            st.sun = sun;
        }
        for i in 0..MAX_CASCADES {
            st.want[i] = if i < n {
                let pts: Vec<Vec3> = cascades::slice_points(&frame, sp[i], sp[i + 1], ymin, ymax).into_iter().map(|p| rot.transform_point3(p)).collect();
                cascades::Need::from_points(&pts, reach).map(|nd| cascades::fit(st.want[i], &nd, res))
            } else {
                None
            };
        }
        let mut todo: Vec<usize> = (0..n).filter(|&i| st.want[i].is_some_and(|p| st.dirty[i] || st.drawn[i] != Some((p, sun)))).collect();
        for i in 0..MAX_CASCADES {
            if !todo.contains(&i) {
                st.waited[i] = 0;
            }
        }
        if !st.force && todo.len() > budget {
            // Primero las que ya esperaron 2 cuadros; después, de la más cercana a la más lejana.
            todo.sort_by_key(|&i| (st.waited[i] < 2, i));
            for &i in &todo[budget..] {
                st.waited[i] += 1;
            }
            todo.truncate(budget);
        }
        st.force = false;
        st.pending = (0..n).any(|i| st.waited[i] > 0 && !todo.contains(&i));
        let mut draw = Vec::with_capacity(todo.len());
        for &i in &todo {
            let p = st.want[i].unwrap();
            let m = p.matrix(rot);
            st.drawn[i] = Some((p, sun));
            st.dirty[i] = false;
            st.waited[i] = 0;
            st.planes[i] = Some(frustum_planes(m));
            let max_lod = if p.half() < 1000.0 { SHADOW_MAX_LOD } else { SHADOW_FAR_LOD };
            draw.push((i, m, max_lod));
        }
        // `MOTOR_DEPURAR_SOMBRAS=1`: escribe el plan de las cascadas y tiñe cada una de un color.
        let depurar = std::env::var("MOTOR_DEPURAR_SOMBRAS").is_ok();
        if depurar {
            eprintln!("cascadas: ojo {eye:?} franja {ymin:.0}..{ymax:.0} cortes {:?} alcance {reach:.0} sol {sun:?}", &sp[..=n]);
            for i in 0..n {
                eprintln!("  {i}: quiere {:?} dibujada {:?} sucia {} dibujar {}", st.want[i], st.drawn[i].map(|d| d.0), st.dirty[i], draw.iter().any(|d| d.0 == i));
            }
        }
        let mut g = SombrasGpu::zeroed();
        for i in 0..n {
            if let (Some(_), Some((p, s))) = (st.want[i], st.drawn[i]) {
                g.vp[i] = p.matrix(cascades::light_rotation(s)).to_cols_array_2d();
                g.info[i] = cascades::shader_info(&p, res).into();
            }
        }
        // [render] w: hay mapa de sombra de los personajes (con la matriz de la cascada 0).
        let agentes = self.sombra_agentes && self.agent_count > 0 && st.drawn[0].is_some() && st.want[0].is_some();
        g.misc = [n as f32, 1.0 / res as f32, if depurar { 1.0 } else { 0.0 }, if agentes { 1.0 } else { 0.0 }];
        (g, draw)
    }

    /// Lugares de las cascadas (para el banco de pruebas): medio lado en metros y texel en cm.
    pub fn cascade_sizes(&self) -> Vec<(f32, f32)> {
        let st = self.casc.borrow();
        st.drawn.iter().take(self.quality.cascades as usize).flatten().map(|(p, _)| (p.half(), p.texel(self.quality.resolution) * 100.0)).collect()
    }

    fn write_globals(&self, cam: &OrbitCamera, hour: f32, aspect: f32, budget: usize) -> View {
        let l = lighting(hour);
        // El sol de las sombras se mueve de a un minuto de juego (0,25°): con el día corriendo
        // no hay que rehacer las cascadas en cada cuadro.
        let sun = lighting((hour * 60.0).round() / 60.0).sun_dir;
        let view_proj = cam.proj(aspect) * cam.view();
        let (sg, draw) = self.plan_cascades(cam, aspect, sun, budget);
        self.queue.write_buffer(&self.sombras, 0, bytemuck::bytes_of(&sg));
        if sg.misc[3] > 0.5 {
            self.queue.write_buffer(&self.agent_vp, 0, bytemuck::cast_slice(&sg.vp[0])); // [render]
        }
        for (i, m, _) in &draw {
            self.queue.write_buffer(&self.cascade_vp, *i as u64 * 256, bytemuck::cast_slice(&m.to_cols_array()));
        }
        let g = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            light_view_proj: sg.vp[0],
            // [render] w: reloj del fundido entre niveles (s desde que se creó el renderer).
            cam_pos: cam.eye().extend(self.reloj_fijo.unwrap_or_else(|| self.reloj.elapsed().as_secs_f32())).into(),
            sun_dir: l.sun_dir.extend(0.0).into(),
            // [entorno amplio] Los .w libres llevan el alcance del terreno (ver `reach`).
            sun_color: l.sun_color.extend(self.reach[2]).into(),
            sky_top: l.sky_top.extend(self.reach[0]).into(),
            sky_horizon: l.sky_horizon.extend(self.reach[1]).into(),
            // Niebla: más tenue cuanto más se aleja la cámara, para que la vista del mapa no se lave.
            // Bruma: en un día claro se ven 30 a 40 km sobre el río; desde muy alto, algo más.
            params: Vec4::new(1.0 / (32000.0f32).max(cam.distance * 10.0), l.night, self.time, l.ambient).into(),
            extra: [if !self.smooth_terrain { 0.0 } else if self.voxel_terrain { 2.0 } else { 1.0 }, self.water_level, self.moon, self.wind.0.rem_euclid(360.0).round() * 100.0 + self.wind.1.clamp(0.0, 99.0)],
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
        let agentes = sg.misc[3] > 0.5;
        let firma = {
            let mut h = crate::fxhash::FxHasher::default();
            std::hash::Hasher::write(&mut h, bytemuck::cast_slice(&sg.vp[0]));
            std::hash::Hasher::write_u64(&mut h, self.agent_key ^ (self.agent_count as u64) << 40);
            std::hash::Hasher::finish(&h) | 1
        };
        // La firma se anota al dibujar el mapa (en `encode`): no todas las globales escritas
        // terminan en un cuadro dibujado.
        let agentes_redibujar = agentes && self.agent_shadow_key.get() != firma;
        View { eye: cam.eye(), view_proj, near: (cam.distance * 0.002).clamp(0.1, 20.0), sun, cascades: draw, focal: 1.0 / (cam.fov_y.to_radians() * 0.5).tan(), agentes, agentes_redibujar, agentes_firma: firma }
    }

    /// [render] ¿Sirven estos destinos para una salida de `w × h` con la escala y el SSAO de ahora?
    fn targets_ok(&self, t: Option<&Targets>, w: u32, h: u32) -> bool {
        t.is_some_and(|t| t.out == (w, h) && t.escala == self.escala && t.ssao.is_some() == self.ssao)
    }

    /// Destinos para una salida de `out_w × out_h`: la escena se dibuja a la escala de render.
    fn make_targets(&self, out_w: u32, out_h: u32) -> Targets {
        let escala = self.escala;
        let (width, height) = if escala < 100 { ((out_w * escala).div_ceil(100).max(1), (out_h * escala).div_ceil(100).max(1)) } else { (out_w, out_h) };
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let make = |format, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor { label: None, size, mip_level_count: 1, sample_count: self.msaa, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
                .create_view(&Default::default())
        };
        let msaa = if self.msaa > 1 { Some(make(self.format, wgpu::TextureUsages::RENDER_ATTACHMENT)) } else { None };
        // Con Hi-Z, la profundidad se lee después en un compute shader.
        let depth_usage = if self.hiz_pipes.is_some() || self.ssao { wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::RENDER_ATTACHMENT };
        let depth = make(DEPTH_FORMAT, depth_usage);
        let hiz = self.hiz_pipes.as_ref().map(|hp| {
            let sizes = hiz::level_sizes(width, height);
            let levels = sizes.len() as u32;
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hi-z"),
                size: wgpu::Extent3d { width: sizes[0].0, height: sizes[0].1, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let mip = |i: u32| tex.create_view(&wgpu::TextureViewDescriptor { base_mip_level: i, mip_level_count: Some(1), ..Default::default() });
            let reduce_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &hp.reduce_bgl,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&depth) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mip(0)) }],
            });
            let down_bg = (1..levels)
                .map(|i| {
                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &hp.down_bgl,
                        entries: &[wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mip(i - 1)) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mip(i)) }],
                    })
                })
                .collect();
            let all = tex.create_view(&Default::default());
            let cull_bg = make_cull_bg(&self.device, &self.cull_bgl, &self.cols, &self.args_main2, &self.cull_params_main2, &self.visible, &all);
            HizTargets { levels, reduce_bg, down_bg, sizes, cull_bg }
        });
        // [render] Pasadas de pantalla: SSAO lejano a media resolución y escala de render.
        let p = &self.post;
        let tex = |w: u32, h: u32, format, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor { label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
                .create_view(&Default::default())
        };
        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        let ssao = self.ssao.then(|| {
            let ao_size = (width.div_ceil(2), height.div_ceil(2));
            let ao = tex(ao_size.0, ao_size.1, AO_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
            let bg_ssao = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &p.bgl_ssao,
                entries: &[entry(0, self.globals.as_entire_binding()), entry(1, p.params_scene.as_entire_binding()), entry(2, wgpu::BindingResource::TextureView(&depth))],
            });
            let bg_apply = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &p.bgl_apply,
                entries: &[entry(0, self.globals.as_entire_binding()), entry(1, p.params_scene.as_entire_binding()), entry(2, wgpu::BindingResource::TextureView(&depth)), entry(3, wgpu::BindingResource::TextureView(&ao))],
            });
            SsaoTargets { ao, size: ao_size, bg_ssao, bg_apply }
        });
        let escalado = (escala < 100).then(|| {
            let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
            let color = tex(width, height, self.format, usage);
            let full = tex(out_w, out_h, self.format, usage);
            let bg = |src: &wgpu::TextureView| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &p.bgl_tex,
                    entries: &[entry(1, p.params_out.as_entire_binding()), entry(4, wgpu::BindingResource::TextureView(src)), entry(5, wgpu::BindingResource::Sampler(&p.sampler))],
                })
            };
            let (bg_up, bg_rcas) = (bg(&color), bg(&full));
            EscaladoTargets { color, full, bg_up, bg_rcas }
        });
        Targets { width, height, msaa, depth, hiz, out: (out_w, out_h), escala, ssao, escalado }
    }

    /// [render] Un cuadro entero: la escena (a la escala de render), el SSAO lejano y el realce a
    /// la salida. Con marcas de tiempo, la última pasada cierra la medida (el tiempo de GPU
    /// incluye las pasadas de pantalla).
    fn encode_frame(&self, enc: &mut wgpu::CommandEncoder, t: &Targets, out: &wgpu::TextureView, view: &View, timestamps: Option<&wgpu::QuerySet>) -> FrameStats {
        let scene_out = t.escalado.as_ref().map_or(out, |e| &e.color);
        let st = self.encode(enc, t, scene_out, view, timestamps);
        if t.ssao.is_none() && t.escalado.is_none() {
            return st;
        }
        let p = &self.post;
        self.queue.write_buffer(&p.params_scene, 0, bytemuck::cast_slice(&[t.width as f32, t.height as f32, t.height as f32 * 0.5 * view.focal, 0.0]));
        self.queue.write_buffer(&p.params_out, 0, bytemuck::cast_slice(&[t.out.0 as f32, t.out.1 as f32, 0.0, 0.0]));
        let pasada = |enc: &mut wgpu::CommandEncoder, label, target: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>, pipe: &wgpu::RenderPipeline, bg: &wgpu::BindGroup, ultima: bool| {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: timestamps.filter(|_| ultima).map(|q| wgpu::RenderPassTimestampWrites { query_set: q, beginning_of_pass_write_index: None, end_of_pass_write_index: Some(1) }),
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        };
        if let Some(s) = &t.ssao {
            let _ = s.size;
            pasada(enc, "ssao", &s.ao, wgpu::LoadOp::Clear(wgpu::Color::WHITE), &p.ssao, &s.bg_ssao, false);
            pasada(enc, "ssao aplicar", scene_out, wgpu::LoadOp::Load, &p.apply, &s.bg_apply, t.escalado.is_none());
        }
        if let Some(e) = &t.escalado {
            pasada(enc, "escala: realce", &e.full, wgpu::LoadOp::Clear(wgpu::Color::BLACK), &p.up, &e.bg_up, false);
            pasada(enc, "escala: nitidez", out, wgpu::LoadOp::Clear(wgpu::Color::BLACK), &p.rcas, &e.bg_rcas, true);
        }
        st
    }

    /// Tramos de caras a dibujar en la vista principal: descarte por frustum, orden de cerca a
    /// lejos y solo los grupos de caras que miran a la cámara, unidos en tramos contiguos
    /// (unión de rangos contiguos, como `preview_draws()` de Tisma engine). Devuelve también
    /// cuántas columnas pasaron.
    fn main_ranges(&self, view: &View) -> (Vec<DrawArgs>, u32) {
        let vp = frustum_planes(view.view_proj);
        let mut list: Vec<(f32, &Batch)> = self
            .batches
            .iter()
            .chain(self.columns.values())
            .filter(|b| {
                let (lo, hi) = curved_box(view.eye, b.lo, b.hi);
                !b.entra && box_visible(&vp, lo, hi)
            })
            .map(|b| (((b.lo + b.hi) * 0.5 - view.eye).length_squared(), b))
            .collect();
        list.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.lod.cmp(&b.1.lod))
                .then(a.1.lo.x.partial_cmp(&b.1.lo.x).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.1.lo.z.partial_cmp(&b.1.lo.z).unwrap_or(std::cmp::Ordering::Equal))
        });
        let cols = list.len() as u32;
        let mut out = Vec::with_capacity(list.len() * 2);
        for (_, b) in list {
            let vis = visible_dirs(view.eye, b.lo, b.hi);
            push_groups(&mut out, b, &vis);
        }
        (out, cols)
    }

    /// Tramos de un pase de sombras: niveles hasta `max_lod` dentro de la caja de luz, y solo
    /// los grupos de caras que miran al sol. En volúmenes cerrados de vóxeles la superficie más
    /// cercana a la luz siempre es una cara que la mira, así que el mapa no cambia.
    fn shadow_ranges(&self, light_vp: Mat4, sun: Vec3, max_lod: u32) -> Vec<DrawArgs> {
        let lp = frustum_planes(light_vp);
        let vis = sun_dirs(sun);
        let mut out = Vec::new();
        for b in self.batches.iter().chain(self.columns.values()).filter(|b| b.lod <= max_lod && box_visible(&lp, b.lo, b.hi)) {
            push_groups(&mut out, b, &vis);
        }
        out
    }

    fn cull_params(&self, m: Mat4, eye: Vec3, shadow: bool, max_lod: u32, phase: u32, screen: [f32; 4]) -> CullParams {
        CullParams {
            planes: frustum_planes(m).map(|p| p.into()),
            eye: eye.extend(1.0).into(),
            count: self.next_slot,
            shadow: shadow as u32,
            max_lod,
            phase,
            view_proj: m.to_cols_array_2d(),
            screen,
        }
    }

    /// Dibuja las caras de los vóxeles en un pase: con argumentos de la GPU (`gpu_args`),
    /// con los tramos de la CPU en el búfer indirecto (desde `cpu_off`), o uno por uno.
    #[allow(clippy::too_many_arguments)]
    fn draw_voxels(&self, pass: &mut wgpu::RenderPass, gpu_args: Option<&wgpu::Buffer>, slots: u32, indirect: bool, cpu: &[DrawArgs], cpu_off: usize) -> u32 {
        if let Some(a) = gpu_args {
            if slots > 0 {
                pass.multi_draw_indexed_indirect(a, 0, slots * 6);
                return 1;
            }
            return 0;
        }
        if indirect {
            if !cpu.is_empty() {
                pass.multi_draw_indexed_indirect(&self.args_cpu, (cpu_off * std::mem::size_of::<DrawArgs>()) as u64, cpu.len() as u32);
                return 1;
            }
            return 0;
        }
        for a in cpu {
            pass.draw_indexed(0..6, 0, a.first_instance..a.first_instance + a.instance_count);
        }
        cpu.len() as u32
    }

    /// Graba las cascadas de sombra que tocan y la vista principal. `out` es la imagen final (o
    /// el destino del resolve si hay MSAA).
    ///
    /// Con Hi-Z (modo GPU) la vista principal va en dos pases: (A) lo visible en el cuadro
    /// anterior; con esa profundidad se arma la pirámide y se prueba todo lo demás; (B) lo que
    /// resultó visible y faltaba, la flora, los vecinos, el agua y el resaltado.
    fn encode(&self, enc: &mut wgpu::CommandEncoder, targets: &Targets, out: &wgpu::TextureView, view: &View, timestamps: Option<&wgpu::QuerySet>) -> FrameStats {
        let mut st = FrameStats::default();
        let gpu = self.draw_mode == DrawMode::GpuCull && self.cull_pipe.is_some();
        let slots = self.next_slot;
        let hz = if gpu && self.hiz && slots > 0 { targets.hiz.as_ref() } else { None };
        st.hiz = hz.is_some();
        let screen = [targets.width as f32, targets.height as f32, hz.map_or(0.0, |h| h.levels as f32), view.near];
        if gpu && slots > 0 {
            // Descarte en la GPU: un compute shader llena los argumentos indirectos.
            let phase = if hz.is_some() { 1 } else { 0 };
            self.queue.write_buffer(&self.cull_params_main, 0, bytemuck::bytes_of(&self.cull_params(view.view_proj, view.eye, false, 0, phase, screen)));
            if hz.is_some() {
                self.queue.write_buffer(&self.cull_params_main2, 0, bytemuck::bytes_of(&self.cull_params(view.view_proj, view.eye, false, 0, 2, screen)));
            }
            for (i, m, lod) in &view.cascades {
                // En el pase de sombras "eye" lleva la dirección del sol (w = 0).
                self.queue.write_buffer(&self.cull_params_shadow[*i], 0, bytemuck::bytes_of(&self.cull_params(*m, view.sun, true, *lod, 0, screen)));
            }
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("descarte"), timestamp_writes: None });
            cp.set_pipeline(self.cull_pipe.as_ref().unwrap());
            cp.set_bind_group(0, &self.cull_bg_main, &[]);
            cp.dispatch_workgroups(slots.div_ceil(64), 1, 1);
            for (i, _, _) in &view.cascades {
                cp.set_bind_group(0, &self.cull_bg_shadow[*i], &[]);
                cp.dispatch_workgroups(slots.div_ceil(64), 1, 1);
            }
        }
        // Tramos de la CPU (modos directo e indirecto). En indirecto se escriben en un solo
        // búfer: primero los de cada cascada y después los principales.
        let shadow_cpu: Vec<Vec<DrawArgs>> = if gpu { Vec::new() } else { view.cascades.iter().map(|(_, m, lod)| self.shadow_ranges(*m, view.sun, *lod)).collect() };
        let (main_cpu, cols_cpu) = if gpu { (Vec::new(), 0) } else { self.main_ranges(view) };
        let indirect = self.draw_mode == DrawMode::Indirect && self.can_indirect;
        let mut offsets = Vec::with_capacity(shadow_cpu.len());
        let mut main_off = 0;
        if !gpu {
            for s in &shadow_cpu {
                offsets.push(main_off);
                main_off += s.len();
            }
            st.cols_main = cols_cpu;
            st.quads_main = main_cpu.iter().map(|a| a.instance_count as u64).sum();
            st.quads_shadow = shadow_cpu.iter().flatten().map(|a| a.instance_count as u64).sum();
            if indirect {
                let mut all: Vec<DrawArgs> = shadow_cpu.iter().flatten().copied().collect();
                all.extend_from_slice(&main_cpu);
                if !all.is_empty() {
                    self.queue.write_buffer(&self.args_cpu, 0, bytemuck::cast_slice(&all));
                }
            }
        }
        for (k, (i, _, _)) in view.cascades.iter().enumerate() {
            self.shadow_redraws.set(self.shadow_redraws.get() + 1);
            st.shadow_passes += 1;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sombras"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_layers[*i],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.shadow_pipe);
            pass.set_bind_group(0, &self.shadow_vox_bg, &[(*i * 256) as u32]);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
            let cpu: &[DrawArgs] = if gpu { &[] } else { &shadow_cpu[k] };
            st.draws_shadow += self.draw_voxels(&mut pass, gpu.then(|| &self.args_shadow[*i]), slots, indirect, cpu, if gpu { 0 } else { offsets[k] });
        }
        // [render] Sombra proyectada de los personajes y animales (mapa propio, cada cuadro).
        if let (Some(buf), true) = (&self.agent_buf, view.agentes_redibujar) {
            self.agent_shadow_key.set(view.agentes_firma);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sombra de los personajes"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.agent_map, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.agent_cast_pipe);
            pass.set_bind_group(0, &self.agent_cast_bg, &[0]);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..36, 0..self.agent_count);
            st.agent_shadow = true;
        }
        let (view_tex, resolve) = match &targets.msaa {
            Some(m) => (m, Some(out)),
            None => (out, None),
        };
        let last = hz.is_none();
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("principal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_tex,
                    resolve_target: if last { resolve } else { None },
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: if last && resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: if last && targets.ssao.is_none() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } }),
                    stencil_ops: None,
                }),
                timestamp_writes: timestamps.map(|q| wgpu::RenderPassTimestampWrites { query_set: q, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: if last { Some(1) } else { None } }),
                occlusion_query_set: None,
            });
        pass.set_pipeline(&self.sky_pipe);
        pass.set_bind_group(0, &self.shadow_bg, &[]);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.voxel_pipe);
        pass.set_bind_group(0, &self.main_bg, &[]);
        pass.set_vertex_buffer(0, self.quads.slice(..));
        pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
        st.draws_main += self.draw_voxels(&mut pass, gpu.then_some(&self.args_main), slots, indirect, &main_cpu, main_off);
        if let Some(h) = hz {
            drop(pass);
            // Pirámide de profundidad con lo dibujado y segunda pasada del descarte.
            let hp = self.hiz_pipes.as_ref().unwrap();
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("hi-z"), timestamp_writes: None });
            cp.set_pipeline(&hp.reduce);
            cp.set_bind_group(0, &h.reduce_bg, &[]);
            cp.dispatch_workgroups(h.sizes[0].0.div_ceil(8), h.sizes[0].1.div_ceil(8), 1);
            cp.set_pipeline(&hp.down);
            for (l, bg) in h.down_bg.iter().enumerate() {
                let (w, hh) = h.sizes[l + 1];
                cp.set_bind_group(0, bg, &[]);
                cp.dispatch_workgroups(w.div_ceil(8), hh.div_ceil(8), 1);
            }
            cp.set_pipeline(self.cull_pipe.as_ref().unwrap());
            cp.set_bind_group(0, &h.cull_bg, &[]);
            cp.dispatch_workgroups(slots.div_ceil(64), 1, 1);
            drop(cp);
            pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("principal (Hi-Z)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_tex,
                    resolve_target: resolve,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: if targets.ssao.is_none() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } }),
                    stencil_ops: None,
                }),
                timestamp_writes: timestamps.map(|q| wgpu::RenderPassTimestampWrites { query_set: q, beginning_of_pass_write_index: None, end_of_pass_write_index: Some(1) }),
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.voxel_pipe);
            pass.set_bind_group(0, &self.main_bg, &[]);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
            st.draws_main += self.draw_voxels(&mut pass, Some(&self.args_main2), slots, indirect, &[], 0);
        }
        // [render] Fundido entre niveles: las columnas que entran y las que salen, con tramado
        // complementario y en una pasada aparte (técnicas del mod LOD §2.5). Son pocas: tramos de
        // la CPU, uno por grupo de caras que mira a la cámara.
        let t_fundido = std::time::Instant::now();
        if !self.entrantes.is_empty() || !self.salientes.is_empty() {
            let vp = frustum_planes(view.view_proj);
            let entran = self.entrantes.iter().filter_map(|(k, slot, _)| self.columns.get(k).filter(|b| b.slot == *slot && b.entra));
            let mut rangos = Vec::new();
            for b in entran.chain(self.salientes.iter().map(|(b, _)| b)) {
                let (lo, hi) = curved_box(view.eye, b.lo, b.hi);
                if box_visible(&vp, lo, hi) {
                    st.cols_fade += 1;
                    push_groups(&mut rangos, b, &visible_dirs(view.eye, b.lo, b.hi));
                }
            }
            if !rangos.is_empty() {
                pass.set_pipeline(&self.fade_pipe);
                pass.set_bind_group(0, &self.main_bg, &[]);
                pass.set_vertex_buffer(0, self.quads.slice(..));
                pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
                st.quads_fade = rangos.iter().map(|a| a.instance_count as u64).sum();
                st.draws_main += self.draw_voxels(&mut pass, None, 0, false, &rangos, 0);
            }
        }
        crate::perfil::sumar(crate::perfil::Sec::Fundido, t_fundido);
        self.refresh_detail();
        let detail = self.detail_buf.borrow();
        if let Some((buf, n)) = detail.as_ref() {
            pass.set_pipeline(&self.detail_pipe);
            pass.set_bind_group(0, &self.main_bg, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..30, 0..*n);
        }
        if let (Some(buf), true) = (&self.agent_buf, self.agent_count > 0) {
            pass.set_pipeline(&self.agent_pipe);
            pass.set_bind_group(0, &self.main_bg, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            // [personajes] seis caras (las ruedas giran y muestran la de abajo) y las sombras
            pass.draw(0..36, 0..self.agent_count);
            pass.set_pipeline(&self.agent_shadow_pipe);
            pass.draw(0..6, 0..self.agent_count);
        }
        // [agua en vóxeles] El plano del río solo queda como respaldo más allá del último anillo
        // (el shader lo recorta adentro, ver `water.wgsl`); el agua son las caras de las columnas,
        // de lejos a cerca (son transparentes).
        pass.set_pipeline(&self.water_pipe);
        pass.set_bind_group(0, &self.shadow_bg, &[]);
        pass.set_vertex_buffer(0, self.water.slice(..));
        pass.draw(0..(self.water.size() / 20) as u32, 0..1);
        let vp = frustum_planes(view.view_proj);
        let mut agua: Vec<(f32, u32, u32)> = self
            .columns
            .values()
            .chain(self.salientes.iter().map(|(b, _)| b)) // [render] el agua de las que salen sigue hasta liberarlas
            .filter(|b| b.water > 0 && {
                let (lo, hi) = curved_box(view.eye, b.lo, b.hi);
                box_visible(&vp, lo, hi)
            })
            .map(|b| (((b.lo + b.hi) * 0.5 - view.eye).length_squared(), b.start + b.groups[6], b.water))
            .collect();
        if !agua.is_empty() {
            agua.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
            pass.set_pipeline(&self.water_quad_pipe);
            pass.set_bind_group(0, &self.main_bg, &[]);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.set_index_buffer(self.quad_index.slice(..), wgpu::IndexFormat::Uint16);
            if self.can_indirect {
                let args: Vec<DrawArgs> = agua.iter().map(|&(_, first, n)| draw_args(first, n)).collect();
                self.queue.write_buffer(&self.args_agua, 0, bytemuck::cast_slice(&args));
                pass.multi_draw_indexed_indirect(&self.args_agua, 0, args.len() as u32);
                st.draws_main += 1;
            } else {
                for (_, first, n) in &agua {
                    pass.draw_indexed(0..6, 0, *first..*first + *n);
                }
                st.draws_main += agua.len() as u32;
            }
        }
        if self.overlay_count > 0 {
            pass.set_pipeline(&self.overlay_pipe);
            pass.set_bind_group(0, &self.shadow_bg, &[]);
            pass.set_vertex_buffer(0, self.overlay.slice(..));
            pass.draw(0..self.overlay_count, 0..1);
        }
        st
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
        use std::sync::atomic::Ordering as AO;
        self.flush_uploads(); // [render]
        // [lod] Tiempo de GPU del pase principal, leído uno o dos cuadros tarde (sin esperar).
        if let Some((_, _, read, period)) = &self.timestamps {
            self.device.poll(wgpu::Maintain::Poll);
            if self.gpu_read.load(AO::SeqCst) == 2 {
                let v: Vec<u64> = bytemuck::cast_slice(&read.slice(..).get_mapped_range()).to_vec();
                read.unmap();
                self.last_gpu_ms = (v[1] > v[0]).then(|| (v[1] - v[0]) as f64 * *period as f64 / 1e6);
                self.gpu_read.store(0, AO::SeqCst);
            }
        }
        let t_wait = std::time::Instant::now();
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.configure(surface, config);
                surface.get_current_texture().map_err(|e| format!("superficie: {e}"))?
            }
            Err(e) => return Err(format!("superficie: {e}")),
        };
        let mut wait = t_wait.elapsed();
        let (w, h) = (frame.texture.width(), frame.texture.height());
        if !self.targets_ok(self.targets.as_ref(), w, h) {
            self.targets = Some(self.make_targets(w, h));
            self.destinos_rehechos += 1;
        }
        let v = self.write_globals(cam, hour, w as f32 / h as f32, self.shadow_budget);
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        let measure = self.timestamps.is_some() && self.gpu_read.load(AO::SeqCst) == 0;
        let ts = if measure { self.timestamps.as_ref().map(|t| &t.0) } else { None };
        let st = self.encode_frame(&mut enc, self.targets.as_ref().unwrap(), &view, &v, ts);
        if measure {
            let (set, resolve, read, _) = self.timestamps.as_ref().unwrap();
            enc.resolve_query_set(set, 0..2, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, read, 0, 16);
        }
        if let Some(ui) = ui {
            ui(&self.device, &self.queue, &mut enc, &view, (w, h));
        }
        self.last_draws = (st.draws_main, st.draws_shadow);
        self.last_shadow_passes = st.shadow_passes;
        self.upload_frame = self.uploaded_bytes - self.upload_mark;
        self.upload_mark = self.uploaded_bytes;
        self.queue.submit([enc.finish()]);
        if measure {
            self.gpu_read.store(1, AO::SeqCst);
            let st = self.gpu_read.clone();
            self.timestamps.as_ref().unwrap().2.slice(..).map_async(wgpu::MapMode::Read, move |r| st.store(if r.is_ok() { 2 } else { 0 }, AO::SeqCst));
        }
        let t_present = std::time::Instant::now();
        frame.present();
        wait += t_present.elapsed();
        self.last_wait_ms = wait.as_secs_f64() * 1000.0;
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
        self.capture_image(cam, hour, width, height, ui).save(path).map_err(|e| format!("no se pudo guardar {path}: {e}"))
    }

    /// Dibuja la escena (y la interfaz, si hay) y devuelve la imagen.
    pub fn capture_image(&self, cam: &OrbitCamera, hour: f32, width: u32, height: u32, ui: Option<UiHook>) -> image::RgbImage {
        self.flush_uploads(); // [render]
        let v = self.write_globals(cam, hour, width as f32 / height as f32, usize::MAX);
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
        self.encode_frame(&mut enc, &targets, &resolve_v, &v, None);
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
        img
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Una columna lejana cuya caja queda apenas por encima del borde superior de la vista,
    /// pero cuyo terreno se dibuja más abajo por la curvatura, no se puede descartar.
    /// [render] Tramos de lugares tocados para subir juntas las filas de la tabla.
    #[test]
    fn los_lugares_tocados_se_suben_en_tramos() {
        let mut v = vec![40, 3, 5, 3, 4, 20, 100, 12];
        assert_eq!(Subidas::tramos(&mut v, 0), vec![(3, 5), (12, 12), (20, 20), (40, 40), (100, 100)]);
        assert!(v.is_empty());
        let mut v = vec![40, 3, 5, 3, 4, 20, 100, 12];
        assert_eq!(Subidas::tramos(&mut v, 8), vec![(3, 20), (40, 40), (100, 100)]);
    }

    #[test]
    fn el_descarte_tiene_en_cuenta_la_curvatura() {
        let cam = OrbitCamera { target: Vec3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 8.0, distance: 200.0, fov_y: 50.0 };
        let vp = cam.proj(16.0 / 9.0) * cam.view();
        let planes = frustum_planes(vp);
        let eye = cam.eye();
        // Hacia adelante (-z), a 12 km: allí la curvatura baja 11 m.
        let z = eye.z - 12_000.0;
        // Altura del borde superior de la vista a esa distancia (rayo de arriba).
        let (o, d) = cam.ray(640.0, 0.0, 1280.0, 720.0);
        let t = (z - o.z) / d.z;
        let top = o.y + d.y * t;
        // Caja de 4 m de alto que empieza 3 m por encima del borde: sin curvatura no se ve.
        let lo = Vec3::new(-1.0, top + 3.0, z - 1.0);
        let hi = Vec3::new(1.0, top + 7.0, z + 1.0);
        assert!(!box_visible(&planes, lo, hi));
        let (clo, chi) = curved_box(eye, lo, hi);
        assert!(box_visible(&planes, clo, chi), "la caja curvada debería verse");
        assert!(clo.y < lo.y - 10.0);
    }

    /// Caché y límite de las cascadas: la primera vez hay que dibujar todas, pero la ventana
    /// dibuja a lo sumo 2 por cuadro y las demás esperan (y piden otro cuadro); con la cámara
    /// quieta no se rehace ninguna; moverse un poco no las mueve todas; la geometría nueva solo
    /// ensucia las cascadas que la contienen. Necesita un adaptador (lavapipe sirve).
    #[test]
    fn las_cascadas_se_rehacen_solo_cuando_hace_falta() {
        let Ok(r) = Renderer::new_msaa(1) else {
            eprintln!("sin adaptador gráfico: prueba salteada");
            return;
        };
        let mut cam = OrbitCamera { target: Vec3::new(13500.0, 20.0, 16300.0), yaw: 200.0, pitch: 35.0, distance: 200.0, fov_y: 50.0 };
        let passes = |r: &Renderer, cam: &OrbitCamera, budget: usize| r.write_globals(cam, 10.5, 16.0 / 9.0, budget).cascades.len();
        assert_eq!(passes(&r, &cam, 2), 2, "límite de 2 por cuadro");
        assert!(r.casc.borrow().pending, "quedaron cascadas esperando: hace falta otro cuadro");
        assert_eq!(passes(&r, &cam, 2), 2);
        assert!(!r.casc.borrow().pending);
        assert_eq!(passes(&r, &cam, 2), 0, "con la cámara quieta no se rehace nada");
        // Un paso de 20 cm: a lo sumo alguna cascada cambia (histéresis).
        cam.pan(0.2, 0.0);
        assert!(passes(&r, &cam, usize::MAX) <= 1);
        // Algo nuevo lejos de todo (a 20 km) no ensucia ninguna cascada; algo en el centro, sí.
        r.shadow_dirty_box(Vec3::new(-20000.0, 0.0, -20000.0), Vec3::new(-19990.0, 10.0, -19990.0));
        assert_eq!(passes(&r, &cam, usize::MAX), 0);
        r.shadow_dirty_box(cam.target - Vec3::splat(2.0), cam.target + Vec3::splat(2.0));
        assert!(passes(&r, &cam, usize::MAX) >= 1);
        // Pedir rehacer todo ignora el límite.
        r.invalidate_shadows();
        assert_eq!(passes(&r, &cam, 2), ShadowQuality::DEFAULT.cascades as usize);
    }
}
