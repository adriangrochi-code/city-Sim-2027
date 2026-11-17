//! Banco de pruebas reproducible (`--bench carpeta`): mismas escenas y cámaras siempre, para
//! medir antes y después de cada optimización.
//!
//! Para cada vista mide: tiempo hasta tener todo generado y mallado (pared) y el tiempo de CPU
//! repartido entre generación y mallado, triángulos, vértices, bytes en la GPU, llamadas de
//! dibujo, tiempo de CPU por cuadro, tiempo total por cuadro (con lavapipe el "GPU" es la CPU)
//! y memoria. Guarda una captura por vista para comparar la imagen.
//!
//! Además (si el adaptador puede), el **descarte de lo tapado** (Hi-Z, en modo de dibujo GPU):
//! columnas y caras dibujadas con y sin Hi-Z, diferencia de imagen, y un barrido de cámara que
//! compara cuadro a cuadro con y sin Hi-Z (si algo apareciera tarde, se vería ahí). Y las
//! **sombras en cascadas**: tamaño y texel de cada cascada, pases por cuadro con la cámara
//! quieta y moviéndose.

use crate::camera::OrbitCamera;
use crate::render::{DrawMode, FrameStats, Renderer, SHADOW_BUDGET_WINDOW};
use crate::session::{memory_mb, trim_memory};
use crate::store::EditStore;
use crate::stream::{Op, Streamer, GEN_NS, MESH_NS};
use crate::structures;
use crate::terrain::TerrainGen;
use glam::{Vec2, Vec3};
use std::fmt::Write as _;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

struct View {
    name: &'static str,
    slug: &'static str,
    cam: [f32; 5],
    hour: f32,
    /// Hacer el barrido de cámara con y sin Hi-Z en esta vista.
    sweep: bool,
}

/// Escena de muestra (terreno de prueba con la ciudad colonial).
const VIEWS: [View; 5] = [
    View { name: "Ciudad de cerca", slug: "cerca", cam: [13500.0, 16300.0, 150.0, 205.0, 32.0], hour: 10.5, sweep: false },
    View { name: "Península", slug: "peninsula", cam: [13500.0, 16300.0, 1600.0, 200.0, 42.0], hour: 10.5, sweep: false },
    View { name: "Lejos (9 km)", slug: "lejos", cam: [13200.0, 15000.0, 9000.0, 190.0, 30.0], hour: 10.5, sweep: false },
    View { name: "Ciudad de noche", slug: "noche", cam: [13500.0, 16300.0, 450.0, 200.0, 35.0], hour: 20.5, sweep: false },
    View { name: "Calle (nivel del suelo)", slug: "calle", cam: [13440.0, 16240.0, 28.0, 215.0, 7.0], hour: 9.5, sweep: true },
];

/// Mapa real de Montevideo (sin ciudad): paisajes con cerros y montes.
const REAL_VIEWS: [View; 3] = [
    View { name: "Cerro, rasante (tapa la bahía)", slug: "real-rasante", cam: [5924.0, 12270.0, 2500.0, 200.0, 6.0], hour: 16.5, sweep: true },
    View { name: "Miguelete de cerca", slug: "real-miguelete", cam: [10431.2, 9144.2, 45.0, 120.0, 14.0], hour: 10.5, sweep: false },
    View { name: "Cerro", slug: "real-cerro", cam: [5924.0, 12270.0, 1600.0, 200.0, 32.0], hour: 10.5, sweep: false },
];

pub struct Options {
    pub dir: String,
    pub threads: usize,
    pub levels: u32,
    pub msaa: u32,
    pub frames: u32,
    pub size: (u32, u32),
}

/// Fracción de píxeles con diferencia > 8 en algún canal.
fn diff(a: &image::RgbImage, b: &image::RgbImage) -> f64 {
    let n = a.pixels().zip(b.pixels()).filter(|(p, q)| p.0.iter().zip(q.0.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > 8)).count();
    n as f64 / (a.width() * a.height()) as f64
}

fn camera(terrain: &TerrainGen, v: &[f32; 5]) -> OrbitCamera {
    let ground = terrain.height(v[0], v[1]).max(0.0);
    OrbitCamera { target: Vec3::new(v[0], ground, v[1]), yaw: v[3], pitch: v[4], distance: v[2], fov_y: 50.0 }
}

/// Lo que se junta para las tablas del Hi-Z y de las sombras.
struct Extra {
    hiz: Vec<String>,
    csm: Vec<String>,
}

pub fn run(o: &Options) -> Result<(), String> {
    std::fs::create_dir_all(&o.dir).map_err(|e| e.to_string())?;
    let terrain = Arc::new(TerrainGen::new(1726));
    let t0 = Instant::now();
    let edits = structures::scenes::ciudad(&terrain, Vec2::new(13500.0, 16300.0), 3, 7).edits;
    let mut store = EditStore::new(o.levels);
    store.apply(&edits, false);
    let t_scene = t0.elapsed();
    let edit_bytes = store.approx_bytes();
    let edit_voxels = store.voxel_count();
    drop(edits);
    trim_memory();
    let store = Arc::new(RwLock::new(store));
    let (rss_store, _) = memory_mb();
    let mut r = Renderer::new_msaa(o.msaa)?;
    let (rss_gpu, _) = memory_mb();
    println!("Memoria: con las ediciones cargadas {rss_store:.0} MB · con el renderer creado {rss_gpu:.0} MB");
    let q = r.shadow_quality();
    let mut md = String::new();
    let _ = writeln!(md, "Adaptador: {} · dibujo {} · Hi-Z {} · {} hilos · {} niveles · MSAA {}× · {}×{} · {} cuadros por vista", r.adapter_name, r.draw_mode.name(), if r.hiz_supported { "disponible" } else { "no disponible" }, o.threads, o.levels, o.msaa, o.size.0, o.size.1, o.frames);
    let _ = writeln!(md, "Sombras: {} cascadas de {}² ({:.0} MB)", q.cascades, q.resolution, q.bytes() as f64 / 1048576.0);
    let _ = writeln!(md, "Escena ciudad: armada en {:.2?} · {} vóxeles editados guardados en todos los niveles · ediciones ≈ {:.1} MB\n", t_scene, edit_voxels, edit_bytes as f64 / 1048576.0);
    let _ = writeln!(md, "| Vista | Generar+mallar (pared) | CPU generación | CPU mallado | Columnas | Triángulos | Caras | Geometría en GPU | Megabúfer | Subido | Dibujos (principal / sombras) | Columnas / caras dibujadas | Caras de sombra | CPU por cuadro | Total por cuadro (sombra rehecha) | Total por cuadro (sombra en caché) | GPU pase principal | RSS |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    println!("{md}");
    let mut extra = Extra { hiz: Vec::new(), csm: Vec::new() };
    for v in &VIEWS {
        bench_view(o, &mut r, &terrain, &store, v, &mut md, &mut extra)?;
    }
    // Mapa real de Montevideo, sin ediciones.
    let real = Arc::new(TerrainGen::montevideo());
    let empty = Arc::new(RwLock::new(EditStore::new(o.levels)));
    for v in &REAL_VIEWS {
        bench_view(o, &mut r, &real, &empty, v, &mut md, &mut extra)?;
    }
    let (_, peak) = memory_mb();
    let _ = writeln!(md, "\nMemoria máxima del proceso (VmHWM): {peak:.0} MB");
    if !extra.hiz.is_empty() {
        let _ = writeln!(md, "\n### Descarte de lo tapado (Hi-Z, modo de dibujo GPU)\n");
        let _ = writeln!(md, "Columnas y caras de la vista principal con descarte por volumen de visión solo y con Hi-Z (en el segundo cuadro con la cámara quieta; entre paréntesis, las de la segunda pasada). La diferencia de imagen es con y sin Hi-Z (píxeles con diferencia > 8). El barrido mueve y gira la cámara 10 cuadros y compara cada cuadro con y sin Hi-Z.\n");
        let _ = writeln!(md, "| Vista | Columnas (sin → con) | Caras (sin → con) | Caras descartadas | Cuadro sin / con Hi-Z | Diferencia de imagen | Barrido: diferencia máxima |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|");
        for l in &extra.hiz {
            let _ = writeln!(md, "{l}");
        }
    }
    let _ = writeln!(md, "\n### Sombras en cascadas\n");
    let _ = writeln!(md, "Medio lado / texel de cada cascada. Pases por cuadro: con la cámara quieta (caché) y con la cámara moviéndose 30 cuadros (1 m y 0,5° por cuadro), con el límite de la ventana ({SHADOW_BUDGET_WINDOW} cascadas por cuadro). Texels rasterizados por cuadro en movimiento: antes, un mapa de 4096² (16,8 M) en cada cuadro.\n");
    let _ = writeln!(md, "| Vista | Cascadas (medio lado · texel) | Pases con la cámara quieta | Pases por cuadro en movimiento | Texels de sombra por cuadro en movimiento | Cuadro en movimiento |");
    let _ = writeln!(md, "|---|---|---|---|---|---|");
    for l in &extra.csm {
        let _ = writeln!(md, "{l}");
    }
    println!("{}", &md[md.find("\n### ").unwrap_or(0)..]);
    println!("Memoria máxima del proceso (VmHWM): {peak:.0} MB");
    std::fs::write(format!("{}/bench.md", o.dir), md).map_err(|e| e.to_string())?;
    Ok(())
}

fn bench_view(o: &Options, r: &mut Renderer, terrain: &Arc<TerrainGen>, store: &Arc<RwLock<EditStore>>, v: &View, md: &mut String, extra: &mut Extra) -> Result<(), String> {
    let lod_px = std::env::var("MOTOR_LOD_FIJO").is_err();
    r.clear_columns();
    r.maintain();
    trim_memory();
    GEN_NS.store(0, Ordering::SeqCst);
    MESH_NS.store(0, Ordering::SeqCst);
    let center = Vec2::new(v.cam[0], v.cam[1]);
    let mut s = Streamer::new(terrain.clone(), store.clone(), o.levels, o.threads);
    let ts = Instant::now();
    let cam = camera(terrain, &v.cam);
    if lod_px {
        s.update_view(center, cam.eye().y - cam.target.y, cam.fov_y, o.size.1 as f32);
    } else {
        s.update(center);
    }
    let mut upload = Duration::ZERO;
    let up0 = r.uploaded_bytes;
    loop {
        let ops = s.poll(usize::MAX);
        let tu = Instant::now();
        for op in ops {
            match op {
                Op::Set(k, m) => r.set_column(k, &m),
                Op::Remove(k) => r.remove_column(k),
            }
        }
        upload += tu.elapsed();
        if s.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let wall = ts.elapsed();
    drop(s);
    r.maintain();
    let (w, h) = o.size;
    // Dos cuadros de calentamiento (compilación de shaders, primeras reservas).
    for _ in 0..2 {
        r.bench_frame(&cam, v.hour, w, h);
    }
    // Cuadros con todas las cascadas rehechas (el peor caso de la cámara en movimiento).
    let mut acc = FrameStats::default();
    for _ in 0..o.frames {
        r.invalidate_shadows();
        let f = r.bench_frame(&cam, v.hour, w, h);
        acc.cpu_ms += f.cpu_ms;
        acc.total_ms += f.total_ms;
        acc = FrameStats { cpu_ms: acc.cpu_ms, total_ms: acc.total_ms, ..f };
    }
    let n = o.frames.max(1) as f64;
    // Cuadros con la cámara quieta: las cascadas se reutilizan.
    let mut cached = 0.0;
    let mut gpu = 0.0;
    let mut gpu_n = 0;
    let mut still_passes = 0;
    for _ in 0..o.frames {
        let f = r.bench_frame(&cam, v.hour, w, h);
        cached += f.total_ms;
        still_passes += f.shadow_passes;
        if let Some(g) = f.gpu_ms {
            gpu += g;
            gpu_n += 1;
        }
    }
    let path = format!("{}/bench-{}.png", o.dir, v.slug);
    r.capture(&cam, v.hour, w, h, &path)?;
    let (vb, ib) = r.gpu_bytes();
    let (rss, _) = memory_mb();
    let line = format!(
        "| {} | {:.2} s (subida {:.2} s) | {:.2} s | {:.2} s | {} | {:.2} M | {:.2} M | {:.1} MB | {:.0} MB | {:.1} MB | {} / {} | {} / {:.2} M | {:.2} M | {:.1} ms | {:.0} ms | {:.0} ms | {} | {:.0} MB |",
        v.name,
        wall.as_secs_f64(),
        upload.as_secs_f64(),
        GEN_NS.load(Ordering::SeqCst) as f64 / 1e9,
        MESH_NS.load(Ordering::SeqCst) as f64 / 1e9,
        r.column_count(),
        r.triangles() as f64 / 1e6,
        r.quad_count() as f64 / 1e6,
        (vb + ib) as f64 / 1048576.0,
        r.megabuffer_mb(),
        (r.uploaded_bytes - up0) as f64 / 1048576.0,
        acc.draws_main,
        acc.draws_shadow,
        acc.cols_main,
        acc.quads_main as f64 / 1e6,
        acc.quads_shadow as f64 / 1e6,
        acc.cpu_ms / n,
        acc.total_ms / n,
        cached / n,
        if gpu_n > 0 { format!("{:.0} ms", gpu / gpu_n as f64) } else { "—".into() },
        rss
    );
    println!("{line}");
    println!("   alturas {:.2} s · superficie {:.2} s · chunks mallados {}", crate::stream::HEIGHT_NS.swap(0, Ordering::SeqCst) as f64 / 1e9, crate::stream::SURF_NS.swap(0, Ordering::SeqCst) as f64 / 1e9, crate::stream::CHUNKS.swap(0, Ordering::SeqCst));
    let _ = writeln!(md, "{line}");

    // Cascadas: tamaño y pases con la cámara en movimiento (límite de la ventana).
    let sizes: Vec<String> = r.cascade_sizes().iter().map(|(h, t)| if *h >= 1000.0 { format!("{:.1} km · {:.0} cm", h / 1000.0, t) } else { format!("{:.0} m · {:.1} cm", h, t) }).collect();
    r.shadow_budget = SHADOW_BUDGET_WINDOW;
    let mut moving = 0;
    let mut moving_ms = 0.0;
    let mut mc = cam;
    for _ in 0..30 {
        mc.yaw += 0.5;
        mc.pan(0.0, 1.0);
        let f = r.bench_frame(&mc, v.hour, w, h);
        moving += f.shadow_passes;
        moving_ms += f.total_ms;
    }
    r.shadow_budget = usize::MAX;
    let res = r.shadow_quality().resolution as f64;
    extra.csm.push(format!(
        "| {} | {} | {:.1} | {:.2} | {:.1} M | {:.0} ms |",
        v.name,
        sizes.join(" / "),
        still_passes as f64 / n,
        moving as f64 / 30.0,
        moving as f64 / 30.0 * res * res / 1e6,
        moving_ms / 30.0
    ));

    // Hi-Z: en modo GPU, sin y con, misma cámara.
    if r.hiz_supported {
        let (mode, hiz) = (r.draw_mode, r.hiz);
        r.draw_mode = DrawMode::GpuCull;
        let measure = |r: &mut Renderer, on: bool| {
            r.hiz = on;
            r.bench_frame(&cam, v.hour, w, h);
            let mut t = 0.0;
            let mut last = FrameStats::default();
            for _ in 0..o.frames {
                last = r.bench_frame(&cam, v.hour, w, h);
                t += last.total_ms;
            }
            (last, t / n, r.capture_image(&cam, v.hour, w, h, None))
        };
        let (off, t_off, img_off) = measure(r, false);
        let (on, t_on, img_on) = measure(r, true);
        let d = diff(&img_off, &img_on);
        let mut sweep = "—".to_string();
        if v.sweep {
            let mut worst = 0.0f64;
            let mut c = cam;
            for k in 0..10 {
                c.yaw += 4.0;
                c.pan(1.5, 3.0);
                r.hiz = true;
                let a = r.capture_image(&c, v.hour, w, h, None);
                if k == 5 {
                    let _ = a.save(format!("{}/bench-{}-barrido.png", o.dir, v.slug));
                }
                r.hiz = false;
                let b = r.capture_image(&c, v.hour, w, h, None);
                worst = worst.max(diff(&a, &b));
            }
            sweep = format!("{:.3} %", worst * 100.0);
        }
        let culled = if off.quads_main > 0 { 100.0 * (1.0 - on.quads_main as f64 / off.quads_main as f64) } else { 0.0 };
        extra.hiz.push(format!(
            "| {} | {} → {} | {:.2} M → {:.2} M ({:.2} M) | {:.0} % | {:.0} / {:.0} ms | {:.3} % | {} |",
            v.name,
            off.cols_main,
            on.cols_main,
            off.quads_main as f64 / 1e6,
            on.quads_main as f64 / 1e6,
            on.quads_phase2 as f64 / 1e6,
            culled,
            t_off,
            t_on,
            d * 100.0,
            sweep
        ));
        println!("   Hi-Z: {}", extra.hiz.last().unwrap());
        r.draw_mode = mode;
        r.hiz = hiz;
    }
    println!("   Sombras: {}", extra.csm.last().unwrap());
    Ok(())
}
