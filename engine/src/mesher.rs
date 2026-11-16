//! Mallado de chunks: solo caras visibles, unidas con *greedy meshing* y con oclusión
//! ambiental por vértice (el sombreado suave de las esquinas típico de los vóxeles).

use crate::lod::CHUNK;
use crate::palette::{self, Mat, AIR};
use crate::world::Level;
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};

const N: i32 = CHUNK;
const P: i32 = N + 2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// Color sRGB del material; el alfa lleva el número de material (para los dibujos
    /// finos por material del shader y las ventanas que brillan de noche).
    pub color: [u8; 4],
    /// Normal (xyz) y oclusión ambiental (w, 0..127).
    pub normal: [i8; 4],
    pub voxel: f32,
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
    pub fn append(&mut self, other: &Mesh) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }
}

/// Copia el chunk con un borde de un vóxel tomado de los vecinos. Fuera del anillo del
/// nivel se considera aire: así los bordes entre niveles generan "faldones" que tapan grietas.
pub fn padded(level: &Level, key: IVec3) -> Vec<Mat> {
    let mut out = vec![AIR; (P * P * P) as usize];
    let base = key * N - IVec3::ONE;
    let chunk = level.chunks.get(&key);
    for y in 0..P {
        for z in 0..P {
            for x in 0..P {
                let inside = x >= 1 && x <= N && y >= 1 && y <= N && z >= 1 && z <= N;
                let m = if inside {
                    chunk.map(|c| c.get((x - 1) as usize, (y - 1) as usize, (z - 1) as usize)).unwrap_or(AIR)
                } else {
                    level.sample(base + IVec3::new(x, y, z)).unwrap_or(AIR)
                };
                out[(x + z * P + y * P * P) as usize] = m;
            }
        }
    }
    out
}

#[inline]
fn at(v: &[Mat], x: i32, y: i32, z: i32) -> Mat {
    v[(x + z * P + y * P * P) as usize]
}

/// Malla de un chunk a partir de su copia con borde. `origin` en metros, `vs` = tamaño del vóxel.
pub fn mesh_chunk(v: &[Mat], origin: Vec3, vs: f32) -> Mesh {
    let mut mesh = Mesh::default();
    let mut mask = vec![0u32; (N * N) as usize];
    for axis in 0..3usize {
        let u_ax = (axis + 1) % 3;
        let v_ax = (axis + 2) % 3;
        for sign in [1i32, -1] {
            let mut n = [0i32; 3];
            n[axis] = sign;
            for slice in 0..N {
                // 1. Máscara de caras visibles con su material y oclusión.
                for j in 0..N {
                    for i in 0..N {
                        let mut c = [0i32; 3];
                        c[axis] = slice;
                        c[u_ax] = i;
                        c[v_ax] = j;
                        let (x, y, z) = (c[0] + 1, c[1] + 1, c[2] + 1);
                        let m = at(v, x, y, z);
                        let key = if m != AIR && at(v, x + n[0], y + n[1], z + n[2]) == AIR {
                            let ao = face_ao(v, [x + n[0], y + n[1], z + n[2]], u_ax, v_ax);
                            m as u32 | (ao << 8) | (1 << 31)
                        } else {
                            0
                        };
                        mask[(i + j * N) as usize] = key;
                    }
                }
                // 2. Unión voraz de rectángulos con la misma clave.
                for j in 0..N {
                    let mut i = 0;
                    while i < N {
                        let key = mask[(i + j * N) as usize];
                        if key == 0 {
                            i += 1;
                            continue;
                        }
                        let mut w = 1;
                        while i + w < N && mask[(i + w + j * N) as usize] == key {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: while j + h < N {
                            for k in 0..w {
                                if mask[(i + k + (j + h) * N) as usize] != key {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        for jj in 0..h {
                            for ii in 0..w {
                                mask[(i + ii + (j + jj) * N) as usize] = 0;
                            }
                        }
                        emit_quad(&mut mesh, origin, vs, axis, u_ax, v_ax, sign, slice, i, j, w, h, key);
                        i += w;
                    }
                }
            }
        }
    }
    mesh
}

/// Oclusión de las 4 esquinas de una cara (0 = muy ocluida, 3 = libre), empaquetada en 8 bits.
fn face_ao(v: &[Mat], p: [i32; 3], u_ax: usize, v_ax: usize) -> u32 {
    let solid = |du: i32, dv: i32| -> u32 {
        let mut q = p;
        q[u_ax] += du;
        q[v_ax] += dv;
        (at(v, q[0], q[1], q[2]) != AIR) as u32
    };
    let mut packed = 0;
    for (k, (du, dv)) in [(-1, -1), (1, -1), (1, 1), (-1, 1)].iter().enumerate() {
        let s1 = solid(*du, 0);
        let s2 = solid(0, *dv);
        let c = solid(*du, *dv);
        let ao = if s1 == 1 && s2 == 1 { 0 } else { 3 - (s1 + s2 + c) };
        packed |= ao << (k * 2);
    }
    packed
}

#[allow(clippy::too_many_arguments)]
fn emit_quad(mesh: &mut Mesh, origin: Vec3, vs: f32, axis: usize, u_ax: usize, v_ax: usize, sign: i32, slice: i32, i: i32, j: i32, w: i32, h: i32, key: u32) {
    let m = (key & 0xff) as Mat;
    let ao = (key >> 8) & 0xff;
    let plane = if sign > 0 { slice + 1 } else { slice };
    let corner = |du: i32, dv: i32| -> [f32; 3] {
        let mut c = [0f32; 3];
        c[axis] = plane as f32;
        c[u_ax] = (i + du) as f32;
        c[v_ax] = (j + dv) as f32;
        [origin.x + c[0] * vs, origin.y + c[1] * vs, origin.z + c[2] * vs]
    };
    let rgb = palette::color(m);
    let color = [rgb[0], rgb[1], rgb[2], m];
    let mut nrm = [0i8; 3];
    nrm[axis] = (sign * 127) as i8;
    // Esquinas en orden antihorario visto desde la normal +eje (u × v = eje).
    let corners = [(0, 0), (w, 0), (w, h), (0, h)];
    let aos = [ao & 3, (ao >> 2) & 3, (ao >> 4) & 3, (ao >> 6) & 3];
    let base = mesh.vertices.len() as u32;
    for (k, (du, dv)) in corners.iter().enumerate() {
        mesh.vertices.push(Vertex {
            pos: corner(*du, *dv),
            color,
            normal: [nrm[0], nrm[1], nrm[2], (aos[k] * 42) as i8],
            voxel: vs,
        });
    }
    // Se elige la diagonal según la oclusión para evitar el defecto de interpolación.
    let flip = aos[0] + aos[2] < aos[1] + aos[3];
    let tris: [u32; 6] = if flip { [1, 2, 3, 1, 3, 0] } else { [0, 1, 2, 0, 2, 3] };
    for t in tris {
        let idx = if sign > 0 { t } else { [0, 3, 2, 1][t as usize] };
        mesh.indices.push(base + idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Vec<Mat> {
        vec![AIR; (P * P * P) as usize]
    }

    #[test]
    fn un_cubo_tiene_seis_caras() {
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, Vec3::ZERO, 0.5);
        assert_eq!(m.triangles(), 12);
    }

    #[test]
    fn el_greedy_une_una_losa_plana() {
        let mut v = empty();
        for z in 1..=N {
            for x in 1..=N {
                v[(x + z * P + 1 * P * P) as usize] = palette::GRASS;
            }
        }
        let m = mesh_chunk(&v, Vec3::ZERO, 0.5);
        // Arriba y abajo: un solo quad cada uno; los 4 bordes: un quad por lado.
        assert_eq!(m.triangles(), 12);
    }

    #[test]
    fn las_normales_apuntan_hacia_afuera() {
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, Vec3::ZERO, 1.0);
        for t in m.indices.chunks(3) {
            let a = Vec3::from(m.vertices[t[0] as usize].pos);
            let b = Vec3::from(m.vertices[t[1] as usize].pos);
            let c = Vec3::from(m.vertices[t[2] as usize].pos);
            let n = m.vertices[t[0] as usize].normal;
            let face = (b - a).cross(c - a);
            let nv = Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32);
            assert!(face.dot(nv) > 0.0, "triángulo con orden invertido");
        }
    }
}
