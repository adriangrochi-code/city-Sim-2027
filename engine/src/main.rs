//! Motor de vóxeles del juego: genera el mapa colonial en vóxeles a la vista, lo malla con
//! niveles de detalle y lo dibuja con wgpu.
//!
//! Uso (modo captura, sin ventana):
//!   motor --shot salida.png [--cam x,z,distancia,giro,inclinación] [--hour 10.5]
//!         [--size 1600x900] [--levels 8]

mod buildings;
mod camera;
mod lod;
mod mesher;
mod palette;
mod render;
mod terrain;
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
}

fn parse_args() -> Args {
    let mut a = Args { shot: None, cam: [13500.0, 16300.0, 420.0, 200.0, 38.0], hour: 10.5, size: (1600, 900), levels: 8 };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = argv.get(i + 1).cloned().unwrap_or_default();
        match argv[i].as_str() {
            "--shot" => a.shot = Some(next),
            "--cam" => {
                for (k, v) in next.split(',').enumerate().take(5) {
                    a.cam[k] = v.parse().unwrap_or(a.cam[k]);
                }
            }
            "--hour" => a.hour = next.parse().unwrap_or(a.hour),
            "--size" => {
                if let Some((w, h)) = next.split_once('x') {
                    a.size = (w.parse().unwrap_or(1600), h.parse().unwrap_or(900));
                }
            }
            "--levels" => a.levels = next.parse().unwrap_or(8),
            "--help" | "-h" => {
                println!("motor --shot salida.png [--cam x,z,distancia,giro,inclinación] [--hour 10.5] [--size 1600x900] [--levels 8]");
                std::process::exit(0);
            }
            other => eprintln!("argumento desconocido: {other}"),
        }
        i += 2;
    }
    a
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
                            let origin = Vec3::new(k.x as f32, k.y as f32, k.z as f32) * (CHUNK as f32 * vs);
                            (level.spec.lod, mesher::mesh_chunk(&pad, origin, vs))
                        })
                        .filter(|(_, m)| !m.indices.is_empty())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

fn main() {
    let args = parse_args();
    let plaza = Vec2::new(13500.0, 16300.0);
    let focus = Vec2::new(args.cam[0], args.cam[1]);

    let t0 = Instant::now();
    let terrain = TerrainGen::new(1726);
    let mut edits = buildings::colonial_city(&terrain, plaza, 3, 7);
    buildings::vegetation(&terrain, &mut edits, plaza, 1800.0, 300.0, 11);
    let t_city = t0.elapsed();

    let levels: Vec<LevelSpec> = (0..args.levels).map(|lod| LevelSpec { lod }).collect();
    let t1 = Instant::now();
    let world = World::generate(terrain, &edits, &levels, focus);
    let t_gen = t1.elapsed();

    let t2 = Instant::now();
    let meshes = mesh_world(&world);
    let t_mesh = t2.elapsed();

    let tris: usize = meshes.iter().map(|(_, m)| m.triangles()).sum();
    println!("Ciudad y vegetación: {} vóxeles editados en {:.2?}", edits.voxels.len(), t_city);
    println!("Mundo: {} chunks en {} niveles, generado en {:.2?}", world.chunk_count(), world.levels.len(), t_gen);
    for l in &world.levels {
        let lt: usize = meshes.iter().filter(|(lod, _)| *lod == l.spec.lod).map(|(_, m)| m.triangles()).sum();
        println!("  nivel {} (vóxel {} m, chunk {} m): {} columnas, {} chunks, {} triángulos", l.spec.lod, l.spec.voxel(), l.spec.chunk_m(), l.columns.len(), l.chunks.len(), lt);
    }
    println!("Mallado: {} triángulos en {:.2?}", tris, t_mesh);

    if let Some(path) = args.shot {
        let mut renderer = match render::Renderer::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
        println!("Adaptador: {}", renderer.adapter_name);
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
