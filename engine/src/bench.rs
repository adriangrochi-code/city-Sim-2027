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
    /// [lod] Preset de calidad (`--calidad`): anillo, sombras, hilos, subida y caché.
    pub calidad: Option<crate::autoajuste::Calidad>,
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
    agentes: Vec<String>, // [personajes]
    hiz: Vec<String>,
    csm: Vec<String>,
    /// [render] Fundido, SSAO lejano y escala de render (técnicas del mod LOD, fases 4 y 6).
    efectos: Vec<String>,
    /// [lod] Una fila por vista para `bench.json`.
    json: Vec<serde_json::Value>,
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
    // [lod] Sombras del preset; la pirámide del relieve se arma antes de medir (en el juego se
    // arma al empezar la partida, mientras carga la escena).
    if let Some(p) = o.calidad.and_then(|c| c.preset()) {
        r.set_shadow_quality(crate::render::ShadowQuality { cascades: p.sombras.0, resolution: p.sombras.1 });
    }
    let t_relieve = Instant::now();
    crate::relieve::obtener(&terrain);
    println!("Pirámide del relieve (mapa de prueba) armada en {:.2?}", t_relieve.elapsed());
    let (rss_gpu, _) = memory_mb();
    println!("Memoria: con las ediciones cargadas {rss_store:.0} MB · con el renderer creado {rss_gpu:.0} MB");
    let q = r.shadow_quality();
    let mut md = String::new();
    let _ = writeln!(md, "Versión: {}\n", crate::version::completa());
    let _ = writeln!(md, "Calidad: {}\n", o.calidad.map_or("sin preset (opciones del banco)", |c| c.nombre()));
    let _ = writeln!(md, "Adaptador: {} · dibujo {} · Hi-Z {} · {} hilos · {} niveles · MSAA {}× · {}×{} · {} cuadros por vista", r.adapter_name, r.draw_mode.name(), if r.hiz_supported { "disponible" } else { "no disponible" }, o.threads, o.levels, o.msaa, o.size.0, o.size.1, o.frames);
    let _ = writeln!(md, "Sombras: {} cascadas de {}² ({:.0} MB)", q.cascades, q.resolution, q.bytes() as f64 / 1048576.0);
    let _ = writeln!(md, "Escena ciudad: armada en {:.2?} · {} vóxeles editados guardados en todos los niveles · ediciones ≈ {:.1} MB\n", t_scene, edit_voxels, edit_bytes as f64 / 1048576.0);
    let _ = writeln!(md, "| Vista | Generar+mallar (pared) | CPU generación | CPU mallado | Columnas | Generadas (ocultas) | Triángulos | Caras | Geometría en GPU | Megabúfer | Subido | Dibujos (principal / sombras) | Columnas / caras dibujadas | Caras de sombra | CPU por cuadro | Total por cuadro (sombra rehecha) | Total por cuadro (sombra en caché) | GPU pase principal | RSS |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    println!("{md}");
    let mut extra = Extra { hiz: Vec::new(), csm: Vec::new(), efectos: Vec::new(), agentes: Vec::new(), json: Vec::new() };
    let mut json = serde_json::json!({ "version": crate::version::completa(), "adaptador": r.adapter_name.clone(), "hilos": o.threads, "niveles": o.levels, "tamano": [o.size.0, o.size.1] });
    // [lod] `MOTOR_BANCO=streaming`: solo la parte de streaming (para medir rápido).
    // `MOTOR_BANCO_VISTAS=peninsula,calle`: solo esas vistas (y sin la parte de streaming).
    let solo_streaming = std::env::var("MOTOR_BANCO").is_ok_and(|v| v == "streaming");
    let vistas: Option<Vec<String>> = std::env::var("MOTOR_BANCO_VISTAS").ok().map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let quiero = |v: &View| !solo_streaming && vistas.as_ref().is_none_or(|l| l.iter().any(|s| s == v.slug));
    for v in VIEWS.iter().filter(|v| quiero(v)) {
        bench_view(o, &mut r, &terrain, &store, v, &mut md, &mut extra)?;
    }
    // Mapa real de Montevideo, sin ediciones.
    let real = Arc::new(TerrainGen::montevideo());
    crate::relieve::obtener(&real);
    let empty = Arc::new(RwLock::new(EditStore::new(o.levels)));
    for v in REAL_VIEWS.iter().filter(|v| quiero(v)) {
        bench_view(o, &mut r, &real, &empty, v, &mut md, &mut extra)?;
    }
    // [lod] Streaming (Fase 0 del plan de técnicas del mod LOD).
    let t_stream = Instant::now();
    let stream = if vistas.is_none() { bench_streaming(o, &mut r, &terrain, &store)? } else { StreamReport { md: String::new(), json: serde_json::Value::Null } };
    println!("{}", stream.md);
    println!("   (streaming medido en {:.0} s)", t_stream.elapsed().as_secs_f64());
    json["streaming"] = stream.json;
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
    // [render]
    if !extra.efectos.is_empty() {
        let _ = writeln!(md, "\n### Fundido entre niveles, SSAO lejano y escala de render\n");
        let _ = writeln!(md, "GPU del pase principal (marcas de tiempo; con lavapipe valen las proporciones), mínimo de {} cuadros con la sombra en caché. Fundido: las 32 y las 128 columnas más cercanas entrando a la vez (el peor caso del vuelo: al recentrar se funden unas decenas). Diferencias de imagen contra la base (píxeles con diferencia > 8).\n", EFECTOS_CUADROS);
        let _ = writeln!(md, "| Vista | Base | Fundido 32 / 128 columnas | SSAO (diferencia de imagen) | Escala 67 % (diferencia) | Prueba \"apagar solo si pierde\" |");
        let _ = writeln!(md, "|---|---|---|---|---|---|");
        for l in &extra.efectos {
            let _ = writeln!(md, "{l}");
        }
    }
    // [personajes]
    if !extra.agentes.is_empty() {
        let _ = writeln!(md, "\n### Personajes y animales (multitud de prueba)\n");
        let _ = writeln!(md, "2500 agentes (el tope de dibujo) en una grilla de 6 m alrededor de la vista: 70 % a pie, jinetes, carretas y carros. Mínimo de los cuadros, con y sin los modelos de micro-vóxeles (a menos de 110 m).\n");
        let _ = writeln!(md, "| Vista | Sin agentes | Con la multitud |");
        let _ = writeln!(md, "|---|---|---|");
        for l in &extra.agentes {
            let _ = writeln!(md, "{l}");
        }
    }
    // [/personajes]
    println!("{}", &md[md.find("\n### ").unwrap_or(0)..]);
    println!("Memoria máxima del proceso (VmHWM): {peak:.0} MB");
    md.push_str(&stream.md);
    std::fs::write(format!("{}/bench.md", o.dir), md).map_err(|e| e.to_string())?;
    json["vistas"] = serde_json::Value::Array(extra.json.clone());
    json["memoria_max_mb"] = serde_json::json!(peak);
    std::fs::write(format!("{}/bench.json", o.dir), serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
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
    let mut s = new_streamer(o, terrain, store);
    let ts = Instant::now();
    let cam = camera(terrain, &v.cam);
    if lod_px {
        crate::session::view_to_streamer(&mut s, &cam, o.size.1 as f32);
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
                Op::Set(k, m) | Op::Entra(k, m) => r.set_column(k, &m),
                Op::Remove(k) | Op::Sale(k) => r.remove_column(k),
            }
        }
        upload += tu.elapsed();
        if s.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let wall = ts.elapsed();
    let (built, hidden) = (s.stats.columns_built, s.stats.hidden_skipped);
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
        "| {} | {:.2} s (subida {:.2} s) | {:.2} s | {:.2} s | {} | {} ({}) | {:.2} M | {:.2} M | {:.1} MB | {:.0} MB | {:.1} MB | {} / {} | {} / {:.2} M | {:.2} M | {:.1} ms | {:.0} ms | {:.0} ms | {} | {:.0} MB |",
        v.name,
        wall.as_secs_f64(),
        upload.as_secs_f64(),
        GEN_NS.load(Ordering::SeqCst) as f64 / 1e9,
        MESH_NS.load(Ordering::SeqCst) as f64 / 1e9,
        r.column_count(),
        built,
        hidden,
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
    extra.json.push(serde_json::json!({
        "vista": v.slug, "generar_s": wall.as_secs_f64(), "subida_s": upload.as_secs_f64(), "columnas": r.column_count(), "columnas_generadas": built, "ocultas": hidden,
        "caras": r.quad_count(), "bytes_subidos": r.uploaded_bytes - up0, "cpu_cuadro_ms": acc.cpu_ms / n, "total_cuadro_ms": acc.total_ms / n, "total_cache_ms": cached / n,
    }));
    println!("   columnas generadas {built} · ocultas por relieve {hidden}");

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
    if matches!(v.slug, "peninsula" | "calle" | "real-cerro" | "real-rasante") {
        bench_efectos(o, r, &cam, v, extra)?;
    }
    // [personajes] Multitud de 2500 agentes alrededor de la vista: sin agentes, solo cajas
    // de siempre y con los modelos de micro-vóxeles cerca (como en el juego).
    if matches!(v.slug, "cerca" | "calle") {
        use crate::city::agentes::dibujo::{multitud_con, CERCA, MAX_DIBUJADOS};
        let medir = |r: &mut Renderer| {
            r.bench_frame(&cam, v.hour, w, h);
            (0..o.frames.max(3)).map(|_| r.bench_frame(&cam, v.hour, w, h).total_ms).fold(f64::INFINITY, f64::min)
        };
        r.set_agents(&[]);
        let sin = medir(r);
        let mut fila = Vec::new();
        for (nombre, cerca) in [("cajas", 0.0), ("micro-vóxeles", CERCA)] {
            let t0 = Instant::now();
            let cajas = multitud_con(terrain, center, MAX_DIBUJADOS, cerca);
            let armar = t0.elapsed();
            r.set_agents(&cajas);
            let con = medir(r);
            // [render] Lo mismo sin la sombra proyectada de los personajes (costo del mapa propio).
            r.sombra_agentes = false;
            let sin_sombra = medir(r);
            if cerca > 0.0 {
                let _ = r.capture_image(&cam, v.hour, w, h, None).save(format!("{}/bench-{}-multitud-sin-sombra.png", o.dir, v.slug));
            }
            r.sombra_agentes = true;
            if cerca > 0.0 {
                let _ = r.capture_image(&cam, v.hour, w, h, None).save(format!("{}/bench-{}-multitud.png", o.dir, v.slug));
            }
            fila.push(format!("{nombre}: {} instancias ({} KB), armadas en {armar:.2?}, cuadro {con:.1} ms (sin sombra proyectada {sin_sombra:.1} ms)", cajas.len(), cajas.len() * std::mem::size_of::<crate::detalle::Caja>() / 1024));
        }
        r.set_agents(&[]);
        let l = format!("| {} | {sin:.1} ms | {} |", v.name, fila.join(" · "));
        println!("   Personajes: {l}");
        extra.agentes.push(l);
    }
    // [/personajes]
    Ok(())
}

// --- [render] Efectos (fundido, SSAO, escala) ----------------------------------------------------

const EFECTOS_CUADROS: usize = 5;

/// GPU del pase principal (mínimo de unos cuadros con la sombra en caché; sin marcas de tiempo,
/// el total del cuadro).
fn gpu_min(r: &mut Renderer, cam: &OrbitCamera, hour: f32, w: u32, h: u32) -> f64 {
    r.bench_frame(cam, hour, w, h);
    (0..EFECTOS_CUADROS).map(|_| { let f = r.bench_frame(cam, hour, w, h); f.gpu_ms.unwrap_or(f.total_ms) }).fold(f64::INFINITY, f64::min)
}

fn bench_efectos(o: &Options, r: &mut Renderer, cam: &OrbitCamera, v: &View, extra: &mut Extra) -> Result<(), String> {
    let (w, h) = o.size;
    let (ssao0, esc0) = (r.ssao, r.escala);
    r.ssao = false;
    r.escala = 100;
    let base = gpu_min(r, cam, v.hour, w, h);
    let img_base = r.capture_image(cam, v.hour, w, h, None);
    // Fundido: las columnas más cercanas entrando a la vez (a mitad de camino).
    let mut fundido = Vec::new();
    for n in [32, 128] {
        r.forzar_fundido(cam.target, n);
        fundido.push(gpu_min(r, cam, v.hour, w, h));
        if n == 32 {
            let _ = r.capture_image(cam, v.hour, w, h, None).save(format!("{}/bench-{}-fundido.png", o.dir, v.slug));
        }
        r.terminar_fundidos();
        r.maintain();
    }
    let quieto = diff(&img_base, &r.capture_image(cam, v.hour, w, h, None));
    // SSAO lejano.
    r.ssao = true;
    let ssao = gpu_min(r, cam, v.hour, w, h);
    let img = r.capture_image(cam, v.hour, w, h, None);
    let _ = img.save(format!("{}/bench-{}-ssao.png", o.dir, v.slug));
    let d_ssao = diff(&img_base, &img);
    r.ssao = false;
    // Escala de render al 67 %.
    r.escala = 67;
    let esc = gpu_min(r, cam, v.hour, w, h);
    let img = r.capture_image(cam, v.hour, w, h, None);
    let _ = img.save(format!("{}/bench-{}-escala67.png", o.dir, v.slug));
    let d_esc = diff(&img_base, &img);
    let prueba = crate::autoajuste::PruebaEscalado::decidir(&[base, base], &[esc, esc]);
    r.escala = 100;
    let pct = |x: f64| format!("{x:.1} ms ({:+.1} %)", 100.0 * (x / base - 1.0));
    let l = format!(
        "| {} | {base:.1} ms | {} / {} (quieto después: {:.3} %) | {} ({:.2} %) | {} ({:.1} %) | {} |",
        v.name,
        pct(fundido[0]),
        pct(fundido[1]),
        quieto * 100.0,
        pct(ssao),
        d_ssao * 100.0,
        pct(esc),
        d_esc * 100.0,
        if prueba { "se queda" } else { "se apaga (no gana > 3 %)" }
    );
    println!("   Efectos: {l}");
    extra.efectos.push(l);
    r.ssao = ssao0;
    r.escala = esc0;
    Ok(())
}

// --- [lod] Streaming: vuelo, zoom de ida y vuelta, teletransporte y salto al recentrar ------
//
// Fase 0 del plan de `docs/sistemas/tecnicas-minecraft-lod.md` (§2.18). Mide el streaming con
// el presupuesto de subida de la ventana y cuadros a ritmo de 60 Hz (el hilo principal duerme
// lo que sobra del cuadro, como con vsync), sin dibujar: el "cuadro" es el trabajo de CPU del
// hilo principal en el streaming (recentrar, recibir y subir mallas). Con lavapipe valen las
// proporciones, no los números absolutos.

/// Percentiles de una serie de tiempos (ms).
#[derive(Clone, Copy, Default)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    /// Media del 1 % de cuadros más lentos.
    pub peor1: f64,
    pub max: f64,
    pub media: f64,
}

pub fn percentiles(v: &[f64]) -> Percentiles {
    if v.is_empty() {
        return Percentiles::default();
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| s[((s.len() - 1) as f64 * q).round() as usize];
    let n1 = (s.len() / 100).max(1);
    Percentiles { p50: at(0.5), p95: at(0.95), p99: at(0.99), peor1: s[s.len() - n1..].iter().sum::<f64>() / n1 as f64, max: s[s.len() - 1], media: s.iter().sum::<f64>() / s.len() as f64 }
}

impl Percentiles {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "p50": self.p50, "p95": self.p95, "p99": self.p99, "peor_1": self.peor1, "max": self.max, "media": self.media })
    }
}

/// Tiempo de CPU del hilo que llama (ms). Con la máquina cargada (otros procesos
/// compilando), el tiempo de pared de un cuadro incluye lo que el sistema le quitó al hilo;
/// el de CPU no.
pub fn cpu_ms() -> f64 {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct Timespec {
            s: i64,
            ns: i64,
        }
        extern "C" {
            fn clock_gettime(clk: i32, t: *mut Timespec) -> i32;
        }
        let mut t = Timespec { s: 0, ns: 0 };
        // CLOCK_THREAD_CPUTIME_ID = 3
        unsafe {
            clock_gettime(3, &mut t);
        }
        t.s as f64 * 1000.0 + t.ns as f64 / 1.0e6
    }
    #[cfg(not(target_os = "linux"))]
    {
        0.0
    }
}

/// Ritmo de los cuadros simulados (60 Hz).
const FRAME: Duration = Duration::from_micros(16_667);

/// Un cuadro como en la ventana: recentrar los anillos según la cámara y subir lo terminado con
/// el presupuesto de la ventana. Devuelve (trabajo total, subida) en ms y los bytes subidos.
/// [render] También devuelve el tiempo de CPU del hilo en el cuadro (cuarto valor).
fn window_step(s: &mut Streamer, r: &mut Renderer, terrain: &TerrainGen, cam: &mut OrbitCamera, view_h: f32) -> (f64, f64, u64, f64) {
    let c0 = cpu_ms();
    let t0 = Instant::now();
    cam.target.y = terrain.height(cam.target.x, cam.target.z).max(0.0);
    crate::session::view_to_streamer(s, cam, view_h);
    let tv = t0.elapsed().as_secs_f64() * 1000.0;
    let up = crate::session::upload_ops(s, r, crate::session::WINDOW_VERTEX_BUDGET);
    let (e, sa) = r.fundidos(); // [render]
    FUNDIDOS_MAX.fetch_max(e + sa, Ordering::Relaxed);
    FUNDIDOS_TOTAL.fetch_add(e + sa, Ordering::Relaxed);
    if std::env::var("MOTOR_BANCO_DETALLE").is_ok() && (t0.elapsed().as_secs_f64() * 1000.0 > 6.0 || cpu_ms() - c0 > 1.0) {
        println!("cuadro lento (cpu {:.2}): vista {tv:.2} · poll {:.2} · maintain {:.2} · subir {:.2} ({} órdenes, {} KB)", cpu_ms() - c0, up.poll_ms, up.maintain_ms, up.ms - up.poll_ms - up.maintain_ms, up.ops, up.bytes / 1024);
    }
    (t0.elapsed().as_secs_f64() * 1000.0, up.ms, up.bytes, cpu_ms() - c0)
}

/// [render] Columnas fundiéndose a la vez en el vuelo: máximo y suma por cuadro.
static FUNDIDOS_MAX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static FUNDIDOS_TOTAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn pace(t0: Instant) {
    if let Some(rest) = FRAME.checked_sub(t0.elapsed()) {
        std::thread::sleep(rest);
    }
}

/// Espera (con cuadros de la ventana) hasta que el streamer queda ocioso; devuelve los segundos.
fn settle(s: &mut Streamer, r: &mut Renderer, terrain: &TerrainGen, cam: &mut OrbitCamera, view_h: f32, max_s: f64) -> f64 {
    let t0 = Instant::now();
    while t0.elapsed().as_secs_f64() < max_s {
        let tf = Instant::now();
        window_step(s, r, terrain, cam, view_h);
        if s.idle() {
            break;
        }
        pace(tf);
    }
    t0.elapsed().as_secs_f64()
}

fn look_yaw(dir: Vec2) -> f32 {
    (-dir.x).atan2(-dir.y).to_degrees()
}

pub struct StreamReport {
    pub md: String,
    pub json: serde_json::Value,
}

pub fn bench_streaming(o: &Options, r: &mut Renderer, terrain: &Arc<TerrainGen>, store: &Arc<RwLock<EditStore>>) -> Result<StreamReport, String> {
    let mut md = String::new();
    let view_h = o.size.1 as f32;
    let city = Vec2::new(13500.0, 16300.0);
    let cerro = crate::terrain::CERRO;
    let dir = (cerro - city).normalize();
    let near = |c: Vec2| OrbitCamera { target: Vec3::new(c.x, 0.0, c.y), yaw: look_yaw(dir), pitch: 32.0, distance: 150.0, fov_y: 50.0 };

    // [render] Como en la ventana: con fundido entre niveles (el streamer manda `Entra`/`Sale`).
    // `MOTOR_SIN_FUNDIDO=1`: sin fundido (para medirlo con el mismo binario).
    r.fundir = std::env::var("MOTOR_SIN_FUNDIDO").is_err();
    // 1) Vuelo: zoom 150 m → 3 km → 150 m (4 s cada tramo) y 3 km hacia el cerro a 40 m/s.
    r.clear_columns();
    r.maintain();
    let mut s = new_streamer(o, terrain, store);
    let mut cam = near(city);
    let t_first = settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
    FUNDIDOS_MAX.store(0, Ordering::Relaxed); // [render]
    FUNDIDOS_TOTAL.store(0, Ordering::Relaxed);
    let (b0, c0) = (r.uploaded_bytes, s.stats.columns_built);
    let mut work = Vec::new();
    let mut upload_max = 0.0f64;
    let dt = FRAME.as_secs_f32();
    let zoom_frames = (4.0 / dt) as usize;
    let fly_frames = (3000.0 / 40.0 / dt) as usize;
    let t_flight = Instant::now();
    let mut work_cpu = Vec::new();
    for f in 0..2 * zoom_frames + fly_frames {
        let tf = Instant::now();
        if f < zoom_frames {
            cam.distance = 150.0 * 20f32.powf((f + 1) as f32 / zoom_frames as f32);
        } else if f < 2 * zoom_frames {
            cam.distance = 3000.0 * (1.0 / 20f32).powf((f + 1 - zoom_frames) as f32 / zoom_frames as f32);
        } else {
            let k = (f + 1 - 2 * zoom_frames) as f32 * dt * 40.0;
            let p = city + dir * k;
            cam.target.x = p.x;
            cam.target.z = p.y;
        }
        let (w, u, _, c) = window_step(&mut s, r, terrain, &mut cam, view_h);
        work.push(w);
        work_cpu.push(c);
        upload_max = upload_max.max(u);
        pace(tf);
    }
    let flight_s = t_flight.elapsed().as_secs_f64();
    let pending_end = s.pending();
    let t_idle = settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
    let (bytes, cols) = (r.uploaded_bytes - b0, s.stats.columns_built - c0);
    let (hits, hidden) = (s.stats.cache_hits, s.stats.hidden_skipped);
    let (occl_n, occl_max, occl_hilo) = (s.stats.occl_evals, s.stats.occl_ms_max, s.stats.occl_thread_ms_max);
    let p = percentiles(&work);
    let (fund_max, fund_total) = (FUNDIDOS_MAX.load(Ordering::Relaxed), FUNDIDOS_TOTAL.load(Ordering::Relaxed)); // [render]
    let pc = percentiles(&work_cpu);
    drop(s);
    let _ = writeln!(md, "\n### Streaming: vuelo, zoom de ida y vuelta, teletransporte y salto al recentrar\n");
    let _ = writeln!(md, "Cuadros a 60 Hz sin dibujar: el cuadro es el trabajo de CPU del hilo principal en el streaming (recentrar, recibir y subir mallas con el presupuesto de la ventana). Vuelo: zoom 150 m → 3 km → 150 m (4 s cada tramo) y 3 km hacia el cerro a 40 m/s. Con lavapipe valen las proporciones, no los números absolutos.\n");
    let _ = writeln!(md, "| Prueba | Cuadros | p50 | p95 | p99 | 1 % peor | Peor cuadro | Peor subida | Columnas generadas | De la caché | Ocultas por relieve | Subido | Pendientes al final | Hasta ocioso |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let _ = writeln!(md, "| Vuelo ({flight_s:.0} s; primera carga {t_first:.1} s) | {} | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.1} ms | {:.1} ms | {cols} | {hits} | {hidden} | {:.1} MB | {pending_end} | {t_idle:.1} s |", work.len(), p.p50, p.p95, p.p99, p.peor1, p.max, upload_max, bytes as f64 / 1048576.0);
    // [render] Con otros procesos en la máquina, el tiempo de pared de un cuadro suma lo que el
    // sistema le quitó al hilo; el de CPU del hilo no.
    let _ = writeln!(md, "| Vuelo, solo CPU del hilo principal | {} | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.1} ms | — | — | — | — | total {:.0} ms | — | — |", work_cpu.len(), pc.p50, pc.p95, pc.p99, pc.peor1, pc.max, pc.media * work_cpu.len() as f64);
    let mut json = serde_json::json!({
        "vuelo_cpu_ms": pc.json(),
        "oclusion": { "evaluaciones": occl_n, "peor_ms_principal": occl_max, "peor_ms_hilo": occl_hilo },
        "vuelo": { "cuadros": work.len(), "segundos": flight_s, "primera_carga_s": t_first, "cuadro_ms": p.json(), "peor_subida_ms": upload_max, "columnas_generadas": cols, "de_la_cache": hits, "ocultas": hidden, "bytes_subidos": bytes, "pendientes_al_final": pending_end, "hasta_ocioso_s": t_idle }
    });

    // 2) Zoom de ida y vuelta, esperando a quedar ocioso en cada punta (dos vueltas).
    r.clear_columns();
    r.maintain();
    let mut s = new_streamer(o, terrain, store);
    let mut cam = near(city);
    settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
    let mut vueltas = Vec::new();
    for _ in 0..2 {
        let (c0, h0, b0) = (s.stats.columns_built, s.stats.cache_hits, r.uploaded_bytes);
        let t0 = Instant::now();
        cam.distance = 3000.0;
        settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
        cam.distance = 150.0;
        settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
        vueltas.push((s.stats.columns_built - c0, s.stats.cache_hits - h0, r.uploaded_bytes - b0, t0.elapsed().as_secs_f64()));
    }
    drop(s);
    for (k, v) in vueltas.iter().enumerate() {
        let _ = writeln!(md, "| Zoom 150 m ↔ 3 km, vuelta {} | — | — | — | — | — | — | — | {} | {} | — | {:.1} MB | 0 | {:.1} s |", k + 1, v.0, v.1, v.2 as f64 / 1048576.0, v.3);
    }
    let _ = writeln!(md, "\nFundido entre niveles en el vuelo: hasta {fund_max} columnas a la vez, {:.1} en promedio por cuadro.", fund_total as f64 / work.len().max(1) as f64);
    let _ = writeln!(md, "\nOclusión por relieve en el vuelo: {occl_n} evaluaciones; la peor, {occl_max:.2} ms en el hilo principal (armar las cajas) y {occl_hilo:.2} ms en su hilo (el barrido).");
    let ratio = if vueltas[0].0 > 0 { vueltas[1].0 as f64 / vueltas[0].0 as f64 } else { 0.0 };
    let _ = writeln!(md, "\nZoom de ida y vuelta: la segunda vuelta genera el {:.0} % de las columnas de la primera.", ratio * 100.0);
    json["zoom"] = serde_json::json!(vueltas.iter().map(|v| serde_json::json!({ "columnas_generadas": v.0, "de_la_cache": v.1, "bytes_subidos": v.2, "segundos": v.3 })).collect::<Vec<_>>());

    // 3) Teletransporte: de la vista lejana (9 km) a la calle; hasta lo visible listo y hasta ocioso.
    r.clear_columns();
    r.maintain();
    let mut s = new_streamer(o, terrain, store);
    let mut cam = camera(terrain, &VIEWS[2].cam);
    settle(&mut s, r, terrain, &mut cam, view_h, 240.0);
    let mut cam = camera(terrain, &VIEWS[4].cam);
    let c0 = s.stats.columns_built;
    let t0 = Instant::now();
    let (eye, look) = (Vec2::new(cam.eye().x, cam.eye().z), Vec2::new(cam.target.x - cam.eye().x, cam.target.z - cam.eye().z).normalize());
    // Medio ángulo horizontal de la vista (FOV vertical 50°) más 10° de margen.
    let half = ((25f32.to_radians().tan() * o.size.0 as f32 / o.size.1 as f32).atan() + 10f32.to_radians()).cos();
    let mut t_visible = None;
    let mut work = Vec::new();
    while t0.elapsed().as_secs_f64() < 240.0 {
        let tf = Instant::now();
        work.push(window_step(&mut s, r, terrain, &mut cam, view_h).0);
        if t_visible.is_none() && s.missing_in_view(eye, look, half) == 0 {
            t_visible = Some(t0.elapsed().as_secs_f64());
        }
        if s.idle() {
            break;
        }
        pace(tf);
    }
    let t_all = t0.elapsed().as_secs_f64();
    let t_visible = t_visible.unwrap_or(t_all);
    let tp = percentiles(&work);
    let cols = s.stats.columns_built - c0;
    drop(s);
    let _ = writeln!(md, "\nTeletransporte de «Lejos (9 km)» a «Calle»: lo visible listo en {t_visible:.2} s, todo en {t_all:.2} s, {cols} columnas generadas; cuadro p99 {:.2} ms, peor {:.1} ms.", tp.p99, tp.max);
    json["teletransporte"] = serde_json::json!({ "visible_listo_s": t_visible, "todo_listo_s": t_all, "columnas_generadas": cols, "cuadro_ms": tp.json() });

    // 4) Salto al recentrar: misma cámara, anillos centrados en A y en A + 512 m (un chunk del
    // nivel 6); diferencia de imagen entre las dos (píxeles con diferencia > 8).
    let _ = writeln!(md, "\n| Vista | Salto al recentrar (píxeles cambiados) |\n|---|---|");
    let mut saltos = serde_json::Map::new();
    let (w, h) = o.size;
    for v in [&VIEWS[1], &VIEWS[2]] {
        let cam = camera(terrain, &v.cam);
        let mut imgs = Vec::new();
        for shift in [0.0, 512.0] {
            r.clear_columns();
            r.maintain();
            let mut s = new_streamer(o, terrain, store);
            crate::session::view_to_streamer(&mut s, &cam, view_h);
            if shift > 0.0 {
                s.update_view(Vec2::new(cam.target.x + shift, cam.target.z), (cam.eye().y - cam.target.y).max(1.0), cam.fov_y, view_h);
            }
            loop {
                crate::session::upload_ops(&mut s, r, usize::MAX);
                if s.idle() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            drop(s);
            r.maintain();
            r.terminar_fundidos(); // [render]
            r.bench_frame(&cam, v.hour, w, h);
            imgs.push(r.capture_image(&cam, v.hour, w, h, None));
        }
        let _ = imgs[1].save(format!("{}/bench-{}-recentrado.png", o.dir, v.slug));
        let d = diff(&imgs[0], &imgs[1]);
        let _ = writeln!(md, "| {} | {:.2} % |", v.name, d * 100.0);
        saltos.insert(v.slug.to_string(), serde_json::json!(d));
    }
    json["salto_al_recentrar"] = serde_json::Value::Object(saltos);
    r.terminar_fundidos(); // [render]
    r.fundir = false;
    Ok(StreamReport { md, json })
}

fn new_streamer(o: &Options, terrain: &Arc<TerrainGen>, store: &Arc<RwLock<EditStore>>) -> Streamer {
    let mut s = Streamer::new(terrain.clone(), store.clone(), o.levels, o.threads);
    if let Some(p) = o.calidad.and_then(|c| c.preset()) {
        s.set_ring(p.anillo);
        s.set_concurrency(p.hilos(crate::autoajuste::nucleos()));
        s.ritmo.ms = p.subida_ms as f64;
        s.cache.set_presupuesto((p.cache_mb as usize) << 20);
    }
    // [render] `MOTOR_SUBIDA_MS=1.0`: presupuesto de streaming por cuadro (para medir).
    if let Some(ms) = std::env::var("MOTOR_SUBIDA_MS").ok().and_then(|v| v.parse::<f64>().ok()) {
        s.ritmo.ms = ms;
    }
    s
}
