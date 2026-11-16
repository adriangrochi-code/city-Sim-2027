//! Banco de pruebas reproducible (`--bench carpeta`): mismas escenas y cámaras siempre, para
//! medir antes y después de cada optimización.
//!
//! Para cada vista mide: tiempo hasta tener todo generado y mallado (pared) y el tiempo de CPU
//! repartido entre generación y mallado, triángulos, vértices, bytes en la GPU, llamadas de
//! dibujo, tiempo de CPU por cuadro, tiempo total por cuadro (con lavapipe el "GPU" es la CPU)
//! y memoria. Guarda una captura por vista para comparar la imagen.

use crate::camera::OrbitCamera;
use crate::render::{FrameStats, Renderer};
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
}

const VIEWS: [View; 4] = [
    View { name: "Ciudad de cerca", slug: "cerca", cam: [13500.0, 16300.0, 150.0, 205.0, 32.0], hour: 10.5 },
    View { name: "Península", slug: "peninsula", cam: [13500.0, 16300.0, 1600.0, 200.0, 42.0], hour: 10.5 },
    View { name: "Lejos (9 km)", slug: "lejos", cam: [13200.0, 15000.0, 9000.0, 190.0, 30.0], hour: 10.5 },
    View { name: "Ciudad de noche", slug: "noche", cam: [13500.0, 16300.0, 450.0, 200.0, 35.0], hour: 20.5 },
];

pub struct Options {
    pub dir: String,
    pub threads: usize,
    pub levels: u32,
    pub msaa: u32,
    pub frames: u32,
    pub size: (u32, u32),
}

pub fn run(o: &Options) -> Result<(), String> {
    let lod_px = std::env::var("MOTOR_LOD_FIJO").is_err();
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
    let mut md = String::new();
    let _ = writeln!(md, "Adaptador: {} · dibujo {} · {} hilos · {} niveles · MSAA {}× · {}×{} · {} cuadros por vista", r.adapter_name, r.draw_mode.name(), o.threads, o.levels, o.msaa, o.size.0, o.size.1, o.frames);
    let _ = writeln!(md, "Escena ciudad: armada en {:.2?} · {} vóxeles editados guardados en todos los niveles · ediciones ≈ {:.1} MB\n", t_scene, edit_voxels, edit_bytes as f64 / 1048576.0);
    let _ = writeln!(md, "| Vista | Generar+mallar (pared) | CPU generación | CPU mallado | Columnas | Triángulos | Caras | Geometría en GPU | Megabúfer | Subido | Dibujos (principal / sombras) | CPU por cuadro | Total por cuadro (sombra rehecha) | Total por cuadro (sombra en caché) | GPU pase principal | RSS |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    println!("{md}");
    for v in &VIEWS {
        r.clear_columns();
        r.maintain();
        trim_memory();
        GEN_NS.store(0, Ordering::SeqCst);
        MESH_NS.store(0, Ordering::SeqCst);
        let center = Vec2::new(v.cam[0], v.cam[1]);
        let mut s = Streamer::new(terrain.clone(), store.clone(), o.levels, o.threads);
        let ts = Instant::now();
        let ground0 = terrain.height(v.cam[0], v.cam[1]).max(0.0);
        let cam0 = OrbitCamera { target: Vec3::new(v.cam[0], ground0, v.cam[1]), yaw: v.cam[3], pitch: v.cam[4], distance: v.cam[2], fov_y: 50.0 };
        if lod_px {
            s.update_view(center, cam0.eye().y - ground0, cam0.fov_y, o.size.1 as f32);
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
        let ground = terrain.height(v.cam[0], v.cam[1]).max(0.0);
        let cam = OrbitCamera { target: Vec3::new(v.cam[0], ground, v.cam[1]), yaw: v.cam[3], pitch: v.cam[4], distance: v.cam[2], fov_y: 50.0 };
        // Dos cuadros de calentamiento (compilación de shaders, primeras reservas).
        for _ in 0..2 {
            r.bench_frame(&cam, v.hour, o.size.0, o.size.1);
        }
        // Cuadros con el mapa de sombras rehecho (el caso de la cámara en movimiento).
        let mut acc = FrameStats::default();
        for _ in 0..o.frames {
            r.invalidate_shadows();
            let f = r.bench_frame(&cam, v.hour, o.size.0, o.size.1);
            acc.cpu_ms += f.cpu_ms;
            acc.total_ms += f.total_ms;
            acc.draws_main = f.draws_main;
            acc.draws_shadow = f.draws_shadow;
        }
        let n = o.frames.max(1) as f64;
        // Cuadros con la cámara quieta: el mapa de sombras se reutiliza.
        let mut cached = 0.0;
        let mut gpu = 0.0;
        let mut gpu_n = 0;
        for _ in 0..o.frames {
            let f = r.bench_frame(&cam, v.hour, o.size.0, o.size.1);
            cached += f.total_ms;
            if let Some(g) = f.gpu_ms {
                gpu += g;
                gpu_n += 1;
            }
        }
        let path = format!("{}/bench-{}.png", o.dir, v.slug);
        r.capture(&cam, v.hour, o.size.0, o.size.1, &path)?;
        let (vb, ib) = r.gpu_bytes();
        let (rss, _) = memory_mb();
        let line = format!(
            "| {} | {:.2} s (subida {:.2} s) | {:.2} s | {:.2} s | {} | {:.2} M | {:.2} M | {:.1} MB | {:.0} MB | {:.1} MB | {} / {} | {:.1} ms | {:.0} ms | {:.0} ms | {} | {:.0} MB |",
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
            acc.cpu_ms / n,
            acc.total_ms / n,
            cached / n,
            if gpu_n > 0 { format!("{:.0} ms", gpu / gpu_n as f64) } else { "—".into() },
            rss
        );
        println!("{line}");
        println!("   alturas {:.2} s · superficie {:.2} s · chunks mallados {}", crate::stream::HEIGHT_NS.swap(0, Ordering::SeqCst) as f64 / 1e9, crate::stream::SURF_NS.swap(0, Ordering::SeqCst) as f64 / 1e9, crate::stream::CHUNKS.swap(0, Ordering::SeqCst));
        let _ = writeln!(md, "{line}");
    }
    let (_, peak) = memory_mb();
    let _ = writeln!(md, "\nMemoria máxima del proceso (VmHWM): {peak:.0} MB");
    println!("Memoria máxima del proceso (VmHWM): {peak:.0} MB");
    std::fs::write(format!("{}/bench.md", o.dir), md).map_err(|e| e.to_string())?;
    Ok(())
}
