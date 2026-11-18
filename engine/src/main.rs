//! Motor de vóxeles del juego: genera el mapa colonial en vóxeles a la vista, lo malla con
//! niveles de detalle y lo dibuja con wgpu.
//!
//! Uso:
//!   motor                              ventana con el juego: menú principal, fundar, trazar calles, zonificar
//!   motor --juego                      directo a una partida nueva (sin el menú principal)
//!   motor --sandbox                    ventana con el sandbox de vóxeles (escena colonial)
//!   motor --demo-ciudad carpeta        recorrido del juego sin pantalla (con --window, en la ventana)
//!   motor --shot salida.png [...]      una captura sin ventana (todo el mundo de una vez)
//!   motor --auto-test carpeta          recorrido automático sin pantalla, con capturas
//!   motor --window --auto-test carpeta el mismo recorrido, en la ventana
//!   motor --bench carpeta              banco de pruebas reproducible (tabla y capturas)
//! Opciones: --mapa real (Montevideo, sin la escena), --scene ciudad|catalogo|vacia, --cam x,z,distancia,giro,inclinación,
//!   --hour 10.5, --size 1600x900, --levels 11, --threads N, --msaa 1|4 (por omisión 4×; 1× en la ventana con render por CPU), --frames N,
//!   --sin-suavizar (sin el suavizado de los escalones del terreno)
//!   --draw directo|indirecto|gpu (modo de dibujo), --hiz 0|1 (descarte de lo tapado; por omisión, sí con descarte en GPU)

mod version;
mod app;
mod bench;
mod buildings;
mod city;
mod detalle; // [personajes]
mod fxhash;
mod camera;
mod lod;
mod menu;
mod mesher;
mod palette;
mod raycast;
mod render;
mod sandbox;
mod save;
mod session;
mod store;
mod stream;
mod structures;
mod terrain;
mod trees;
mod tour;
mod ui;
mod world;

use camera::OrbitCamera;
use glam::{IVec3, Vec2, Vec3};
use lod::{LevelSpec, CHUNK};
use mesher::Mesh;
use std::time::Instant;
use terrain::TerrainGen;
use world::World;

struct Args {
    shot: Option<String>,
    cam: [f32; 5],
    hour: f32,
    size: (u32, u32),
    levels: u32,
    scene: String,
    cam_set: bool,
    auto_test: Option<String>,
    window: bool,
    frames: Option<u64>,
    msaa: u32,
    threads: usize,
    smooth: bool,
    bench: Option<String>,
    /// Modo juego (ciudad colonial jugable) en vez del sandbox.
    juego: bool,
    /// `--juego` explícito: entra directo a una partida nueva, sin el menú principal.
    juego_directo: bool,
    levels_set: bool,
    sandbox: bool,
    scene_set: bool,
    /// Capturas y sandbox sobre el mapa real de Montevideo (`--mapa real`).
    real: bool,
}

fn parse_args() -> Args {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(1);
    let mut a = Args {
        shot: None,
        cam: [13500.0, 16300.0, 420.0, 200.0, 38.0],
        hour: 10.5,
        size: (1600, 900),
        levels: crate::lod::LEVELS,
        scene: "ciudad".into(),
        cam_set: false,
        auto_test: None,
        window: false,
        frames: None,
        msaa: 0,
        threads,
        smooth: true,
        bench: None,
        juego: false,
        juego_directo: false,
        levels_set: false,
        sandbox: false,
        scene_set: false,
        real: false,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = argv.get(i + 1).cloned().unwrap_or_default();
        match argv[i].as_str() {
            "--shot" => a.shot = Some(next),
            "--scene" => {
                a.scene = next;
                a.scene_set = true;
            }
            "--demo-ciudad" => {
                a.auto_test = Some(next);
                a.juego = true;
            }
            "--juego" | "--sandbox" => {
                if argv[i] == "--juego" {
                    a.juego = true;
                    a.juego_directo = true;
                } else {
                    a.sandbox = true;
                }
                i += 1;
                continue;
            }
            "--cam" => {
                a.cam_set = true;
                for (k, v) in next.split(',').enumerate().take(5) {
                    a.cam[k] = v.parse().unwrap_or(a.cam[k]);
                }
            }
            "--mapa" => a.real = next == "real",
            "--hour" => a.hour = next.parse().unwrap_or(a.hour),
            "--size" => {
                if let Some((w, h)) = next.split_once('x') {
                    a.size = (w.parse().unwrap_or(1600), h.parse().unwrap_or(900));
                }
            }
            "--levels" => {
                a.levels = next.parse().unwrap_or(crate::lod::LEVELS);
                a.levels_set = true;
            }
            "--auto-test" => a.auto_test = Some(next),
            "--bench" => a.bench = Some(next),
            "--frames" => a.frames = next.parse().ok(),
            "--msaa" => a.msaa = if next == "1" { 1 } else { 4 },
            // Sin --msaa: 4× en capturas y con GPU; 1× en la ventana si el render es por CPU.
            "--threads" => a.threads = next.parse().unwrap_or(a.threads).max(1),
            // Dibujo: directo, indirecto (múltiple, descarte en CPU) o gpu (descarte en GPU).
            "--draw" => {
                let m = match next.as_str() {
                    "directo" => 1,
                    "indirecto" => 2,
                    "gpu" => 3,
                    _ => 0,
                };
                render::DRAW_OVERRIDE.store(m, std::sync::atomic::Ordering::Relaxed);
            }
            // Descarte de lo tapado (Hi-Z): 1 lo fuerza (si el adaptador puede), 0 lo apaga.
            "--hiz" => render::HIZ_OVERRIDE.store(if next == "1" { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed),
            "--gpu-cull" => render::DRAW_OVERRIDE.store(if next == "1" { 3 } else { 2 }, std::sync::atomic::Ordering::Relaxed),
            "--window" => {
                a.window = true;
                i += 1;
                continue;
            }
            "--sin-suavizar" => {
                a.smooth = false;
                i += 1;
                continue;
            }
            "--help" | "-h" => {
                println!("{}", include_str!("main.rs").lines().skip(3).take(13).map(|l| l.trim_start_matches("//!").trim_start_matches(' ')).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => eprintln!("argumento desconocido: {other}"),
        }
        i += 2;
    }
    // Sin escena ni recorrido pedidos, la ventana abre el juego.
    if !a.sandbox && !a.scene_set && a.auto_test.is_none() && a.shot.is_none() && a.bench.is_none() {
        a.juego = true;
    }
    a
}

/// Sesión del juego o del sandbox según los argumentos.
fn new_session(args: &Args) -> session::Session {
    if args.juego {
        let p = city::game_demo::PLAZA;
        let cam = if args.cam_set { args.cam } else { [p.x, p.y, 700.0, 200.0, 45.0] };
        session::Session::new_game(args.levels, args.threads, cam, args.hour)
    } else {
        let cam = if args.cam_set { args.cam } else { session::scene_camera(&args.scene) };
        if args.real {
            let t = std::sync::Arc::new(terrain::TerrainGen::montevideo());
            return session::Session::with_terrain("vacia", t, args.levels, args.threads, cam, args.hour);
        }
        session::Session::new(&args.scene, args.levels, args.threads, cam, args.hour)
    }
}

/// Malla todos los chunks de todos los niveles en paralelo.
fn mesh_world(world: &World) -> Vec<(u32, Mesh)> {
    let mut jobs: Vec<(usize, IVec3)> = Vec::new();
    for (li, level) in world.levels.iter().enumerate() {
        for (&k, ch) in &level.chunks {
            // Un chunk lleno solo se puede saltear si el de arriba también lo está:
            // si no, su cara superior es la superficie del terreno.
            // Lo mismo para los vecinos laterales (laderas empinadas, barrancas).
            let full = |d: IVec3| level.chunks.get(&(k + d)).is_some_and(|a| a.is_full());
            if ch.is_full() && [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].into_iter().all(full) {
                continue;
            }
            jobs.push((li, k));
        }
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let per = jobs.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .chunks(per)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|&(li, k)| {
                            let level = &world.levels[li];
                            let vs = level.spec.voxel();
                            let pad = mesher::padded(level, k);
                            let mut m = mesher::mesh_chunk(&pad, IVec3::ZERO);
                            m.origin = Vec3::new(k.x as f32, k.y as f32, k.z as f32) * (CHUNK as f32 * vs);
                            m.voxel = vs;
                            (level.spec.lod, m)
                        })
                        .filter(|(_, m)| !m.is_empty())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// Memoria máxima del proceso (Linux), en MB.
fn peak_memory_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

/// Recorrido automático sin pantalla.
fn run_headless_tour(args: &Args, dir: &str) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let mut r = render::Renderer::new()?;
    r.smooth_terrain = args.smooth;
    println!("Adaptador: {} · listo en {:.2?} · {} hilos de trabajo", r.adapter_name, t0.elapsed(), args.threads);
    let mut s = new_session(args);
    s.view_h = args.size.1 as f32;
    let steps = if s.game.is_some() { tour::city_steps() } else if args.real { tour::landscape_steps() } else { tour::default_steps() };
    let mut tour = tour::Tour::with_steps(dir, args.size.0, args.size.1, steps);
    let mut first_logged = false;
    loop {
        s.update(1.0 / 30.0);
        s.apply_ops(&mut r, usize::MAX);
        if !first_logged && s.first_frame.is_some() {
            first_logged = true;
            println!("Primeras columnas listas para dibujar a los {:.2?}", s.first_frame.unwrap());
        }
        if tour.step(&mut s, &mut r) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    println!("Resumen del recorrido:");
    for l in &tour.report {
        println!("  {l}");
    }
    println!("Columnas generadas en total: {} · subidas a la GPU: {}", s.streamer.stats.columns_built, s.streamer.stats.uploads);
    Ok(())
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("City Sim 2027 · motor {}", version::completa());
        return;
    }
    println!("City Sim 2027 · motor {}", version::completa());
    let mut args = parse_args();
    let plaza = Vec2::new(13500.0, 16300.0);
    if let Some(dir) = &args.bench {
        let o = bench::Options { dir: dir.clone(), threads: args.threads, levels: args.levels, msaa: if args.msaa == 0 { 4 } else { args.msaa }, frames: args.frames.unwrap_or(5) as u32, size: if args.size == (1600, 900) { (1280, 720) } else { args.size } };
        if let Err(e) = bench::run(&o) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.shot.is_none() {
        let result = match (&args.auto_test, args.window) {
            (Some(dir), false) => run_headless_tour(&args, &dir.clone()),
            _ => {
                // Sin argumentos: menú principal con la escena colonial girando detrás.
                let base = save::data_dir();
                let settings = save::Settings::load(&base);
                if !args.levels_set {
                    args.levels = settings.niveles;
                }
                let main_menu = args.juego && !args.juego_directo && args.auto_test.is_none();
                let menu = menu::Menu::new(base, settings, args.levels, args.threads, main_menu);
                let s = if main_menu { session::Session::new(menu.background_scene, args.levels, args.threads, menu::BACKGROUND_CAM, menu::BACKGROUND_HOUR) } else { new_session(&args) };
                app::run(s, app::Options { size: args.size, msaa: args.msaa, frames: args.frames, tour_dir: args.auto_test.clone() }, menu)
            }
        };
        if let Err(e) = result {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    let t0 = Instant::now();
    let terrain = TerrainGen::new(1726);
    let scene = match args.scene.as_str() {
        "catalogo" => structures::scenes::catalogo(&terrain),
        "ciudad" => structures::scenes::ciudad(&terrain, plaza, 3, 7),
        "vacia" => structures::scenes::SceneOut { edits: buildings::Edits::default(), cam: args.cam, center: plaza },
        other => {
            eprintln!("escena desconocida: {other} (use ciudad, catalogo o vacia)");
            std::process::exit(2);
        }
    };
    if !args.cam_set {
        args.cam = scene.cam;
    }
    let focus = Vec2::new(args.cam[0], args.cam[1]);
    let edits = scene.edits;
    let _ = scene.center;
    let t_city = t0.elapsed();

    let levels: Vec<LevelSpec> = (0..args.levels).map(|lod| LevelSpec { lod }).collect();
    let t1 = Instant::now();
    let world = World::generate(terrain, &edits, &levels, focus);
    let t_gen = t1.elapsed();

    let t2 = Instant::now();
    let meshes = mesh_world(&world);
    let t_mesh = t2.elapsed();

    let tris: usize = meshes.iter().map(|(_, m)| m.triangles()).sum();
    println!("Escena «{}»: {} vóxeles editados en {:.2?}", args.scene, edits.voxel_count(), t_city);
    println!("Mundo: {} chunks en {} niveles, generado en {:.2?}", world.chunk_count(), world.levels.len(), t_gen);
    for l in &world.levels {
        let lt: usize = meshes.iter().filter(|(lod, _)| *lod == l.spec.lod).map(|(_, m)| m.triangles()).sum();
        println!("  nivel {} (vóxel {} m, chunk {} m): {} columnas, {} chunks, {} triángulos", l.spec.lod, l.spec.voxel(), l.spec.chunk_m(), l.columns.len(), l.chunks.len(), lt);
    }
    println!("Mallado: {} triángulos en {:.2?}", tris, t_mesh);
    if let Some(mb) = peak_memory_mb() {
        println!("Memoria máxima hasta el mallado: {mb:.0} MB");
    }

    if let Some(path) = args.shot {
        let mut renderer = match render::Renderer::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
        println!("Adaptador: {}", renderer.adapter_name);
        renderer.smooth_terrain = args.smooth;
        renderer.upload(&meshes);
        let ground = world.terrain.height(focus.x, focus.y).max(0.0);
        let cam = OrbitCamera { target: Vec3::new(focus.x, ground, focus.y), yaw: args.cam[3], pitch: args.cam[4], distance: args.cam[2], fov_y: 50.0 };
        let t3 = Instant::now();
        match renderer.capture(&cam, args.hour, args.size.0, args.size.1, &path) {
            Ok(()) => println!("Captura guardada en {path} (render {:.2?})", t3.elapsed()),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
