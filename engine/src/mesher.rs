//! Mallado de chunks: solo caras visibles, unidas con *greedy meshing* y con oclusión
//! ambiental por vértice (el sombreado suave de las esquinas típico de los vóxeles).

use crate::lod::CHUNK;
use crate::palette::{self, Mat, AIR};
use crate::world::Level;
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};

const N: i32 = CHUNK;
const P: i32 = N + 2;

/// Vértice compacto: 16 bytes (antes 28).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Vertex {
    /// Posición en vóxeles, relativa al origen de la malla (que va aparte, por columna).
    pub pos: [u16; 4],
    /// Color sRGB del material; el alfa lleva el número de material (para los dibujos
    /// finos por material del shader y las ventanas que brillan de noche).
    pub color: [u8; 4],
    /// x: dirección de la cara (0–5, ver [`DIRS`]) + oclusión ambiental (0–3) × 8;
    /// y, z: normal suave de la ladera (x, z) + 128 (solo terreno: así los escalones de un
    /// vóxel no se ven como curvas de nivel oscuras); w: libre.
    pub info: [u8; 4],
}

/// Direcciones de cara en el orden de los grupos de índices: primero las que casi siempre
/// se ven desde una cámara de constructor de ciudades (arriba), al final la de abajo.
/// El shader (`voxel.wgsl`) tiene la misma tabla.
#[cfg_attr(not(test), allow(dead_code))]
pub const DIRS: [[i32; 3]; 6] = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]];

#[inline]
pub fn dir_index(axis: usize, sign: i32) -> usize {
    match (axis, sign > 0) {
        (1, true) => 0,
        (0, true) => 1,
        (0, false) => 2,
        (2, true) => 3,
        (2, false) => 4,
        _ => 5,
    }
}

/// Desempaquetado (para pruebas: el shader hace lo mismo en la GPU).
#[cfg(test)]
impl Vertex {
    pub fn dir(&self) -> usize {
        (self.info[0] & 7) as usize
    }
    pub fn ao(&self) -> u32 {
        (self.info[0] >> 3) as u32
    }
    pub fn normal(&self) -> Vec3 {
        let d = DIRS[self.dir()];
        Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32)
    }
    pub fn smooth(&self) -> Vec3 {
        let x = (self.info[1] as f32 - 128.0) / 127.0;
        let z = (self.info[2] as f32 - 128.0) / 127.0;
        Vec3::new(x, (1.0 - x * x - z * z).max(0.0).sqrt(), z)
    }
}

/// Materiales del terreno natural y del suelo (calles, plaza, huertas).
pub fn is_terrain(m: Mat) -> bool {
    use palette::*;
    matches!(m, GRASS | DIRT | SAND | ROCK | MUD | COBBLE | DIRT_ROAD | PLAZA | GRASS_DRY | GARDEN)
}

/// Altura (índice y en la copia con borde) del vóxel de terreno más alto de cada columna.
fn terrain_tops(v: &[Mat]) -> Vec<i32> {
    let mut tops = vec![-1i32; (P * P) as usize];
    for z in 0..P {
        for x in 0..P {
            for y in (0..P).rev() {
                if is_terrain(at(v, x, y, z)) {
                    tops[(x + z * P) as usize] = y;
                    break;
                }
            }
        }
    }
    tops
}

/// Normal suave en una esquina de la grilla (coordenadas de la copia con borde).
fn smooth_normal(tops: &[i32], cx: i32, cz: i32) -> [i8; 4] {
    let raw = |x: i32, z: i32| tops[(x.clamp(0, P - 1) + z.clamp(0, P - 1) * P) as usize];
    // Altura de referencia: la de las columnas que tocan la esquina. Las columnas sin terreno
    // (fuera del anillo del nivel, o con la superficie por debajo de este chunk) la toman
    // prestada, así los bordes entre niveles no se ven como pendientes enormes.
    let mut sum = 0;
    let mut cnt = 0;
    for (x, z) in [(cx - 1, cz - 1), (cx, cz - 1), (cx - 1, cz), (cx, cz)] {
        let t = raw(x, z);
        if t >= 0 {
            sum += t;
            cnt += 1;
        }
    }
    let base = if cnt > 0 { sum as f32 / cnt as f32 } else { 0.0 };
    let h = |x: i32, z: i32| -> f32 {
        let t = raw(x, z);
        if t >= 0 { t as f32 } else { base }
    };
    let gx = (h(cx, cz - 1) + h(cx, cz) + h(cx + 1, cz - 1) + h(cx + 1, cz)) - (h(cx - 1, cz - 1) + h(cx - 1, cz) + h(cx - 2, cz - 1) + h(cx - 2, cz));
    let gz = (h(cx - 1, cz) + h(cx, cz) + h(cx - 1, cz + 1) + h(cx, cz + 1)) - (h(cx - 1, cz - 1) + h(cx, cz - 1) + h(cx - 1, cz - 2) + h(cx, cz - 2));
    // Cada suma son 4 columnas a 2 vóxeles de distancia: pendiente = g / 4 / 2.
    let n = Vec3::new(-gx / 8.0, 1.0, -gz / 8.0).normalize();
    [(n.x * 127.0) as i8, (n.y * 127.0) as i8, (n.z * 127.0) as i8, 0]
}

/// Malla de una columna (o de un chunk): vértices compactos con posición relativa a
/// `origin`, índices agrupados por dirección de cara y caja envolvente en metros.
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    /// Índices por dirección de cara ([`DIRS`]).
    pub groups: [Vec<u32>; 6],
    /// Origen de la malla en metros y tamaño del vóxel.
    pub origin: Vec3,
    pub voxel: f32,
    /// Caja envolvente en vóxeles relativos al origen (min, max).
    pub lo: [u16; 3],
    pub hi: [u16; 3],
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh { vertices: Vec::new(), groups: Default::default(), origin: Vec3::ZERO, voxel: 1.0, lo: [u16::MAX; 3], hi: [0; 3] }
    }
}

impl Mesh {
    pub fn new(origin: Vec3, voxel: f32) -> Mesh {
        Mesh { origin, voxel, ..Default::default() }
    }
    pub fn index_count(&self) -> usize {
        self.groups.iter().map(|g| g.len()).sum()
    }
    pub fn triangles(&self) -> usize {
        self.index_count() / 3
    }
    pub fn is_empty(&self) -> bool {
        self.index_count() == 0
    }
    /// Une otra malla con el mismo origen.
    pub fn append(&mut self, other: &Mesh) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        for (g, og) in self.groups.iter_mut().zip(other.groups.iter()) {
            g.extend(og.iter().map(|i| i + base));
        }
        for k in 0..3 {
            self.lo[k] = self.lo[k].min(other.lo[k]);
            self.hi[k] = self.hi[k].max(other.hi[k]);
        }
    }
    /// Índices de todos los grupos seguidos y dónde empieza cada grupo (7 valores).
    pub fn indices_grouped(&self) -> (Vec<u32>, [u32; 7]) {
        let mut out = Vec::with_capacity(self.index_count());
        let mut starts = [0u32; 7];
        for (k, g) in self.groups.iter().enumerate() {
            starts[k] = out.len() as u32;
            out.extend_from_slice(g);
        }
        starts[6] = out.len() as u32;
        (out, starts)
    }
    /// Caja envolvente en metros.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let f = |v: [u16; 3]| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32) * self.voxel + self.origin;
        (f(self.lo), f(self.hi))
    }
    /// Posición de un vértice en metros (para pruebas).
    #[cfg(test)]
    pub fn world_pos(&self, v: &Vertex) -> Vec3 {
        Vec3::new(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32) * self.voxel + self.origin
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

/// Malla de un chunk a partir de su copia con borde. `offset` = posición del chunk, en vóxeles,
/// relativa al origen de la malla (las posiciones de los vértices se guardan así).
///
/// Las caras visibles se encuentran con máscaras de bits: una fila de 34 vóxeles a lo largo de
/// x cabe en un `u64`, y "sólido aquí y aire en el vecino" es una operación de bits por fila.
/// Solo se calculan material y oclusión donde realmente hay una cara; después, cada corte se
/// une con el mismo *greedy meshing* de siempre (la salida es idéntica a la versión anterior).
pub fn mesh_chunk(v: &[Mat], offset: IVec3) -> Mesh {
    let mut mesh = Mesh::default();
    let tops = terrain_tops(v);
    // Bits de sólido por fila: rows[y * P + z] tiene el bit x encendido si (x, y, z) no es aire.
    let mut rows = vec![0u64; (P * P) as usize];
    for y in 0..P {
        for z in 0..P {
            let base = (z * P + y * P * P) as usize;
            let mut b = 0u64;
            for x in 0..P as usize {
                b |= ((v[base + x] != AIR) as u64) << x;
            }
            rows[(y * P + z) as usize] = b;
        }
    }
    let inner: u64 = ((1u64 << N) - 1) << 1; // bits 1..=N
    let mut masks = vec![0u32; (N * N * N) as usize];
    let mut counts = [0u32; N as usize];
    for axis in 0..3usize {
        let u_ax = (axis + 1) % 3;
        let v_ax = (axis + 2) % 3;
        for sign in [1i32, -1] {
            let mut n = [0i32; 3];
            n[axis] = sign;
            counts.fill(0);
            // 1. Máscaras de caras visibles de todos los cortes de esta dirección.
            for y in 1..=N {
                for z in 1..=N {
                    let row = rows[(y * P + z) as usize];
                    let faces = match axis {
                        0 => {
                            if sign > 0 {
                                row & !(row >> 1)
                            } else {
                                row & !(row << 1)
                            }
                        }
                        1 => row & !rows[((y + sign) * P + z) as usize],
                        _ => row & !rows[(y * P + z + sign) as usize],
                    } & inner;
                    let mut bits = faces;
                    while bits != 0 {
                        let x = bits.trailing_zeros() as i32;
                        bits &= bits - 1;
                        let (slice, i, j) = match axis {
                            0 => (x - 1, y - 1, z - 1),
                            1 => (y - 1, z - 1, x - 1),
                            _ => (z - 1, x - 1, y - 1),
                        };
                        let m = at(v, x, y, z);
                        let ao = face_ao(v, [x + n[0], y + n[1], z + n[2]], u_ax, v_ax);
                        masks[(slice * N * N + i + j * N) as usize] = m as u32 | (ao << 8) | (1 << 31);
                        counts[slice as usize] += 1;
                    }
                }
            }
            // 2. Unión voraz de rectángulos con la misma clave, corte por corte.
            for slice in 0..N {
                if counts[slice as usize] == 0 {
                    continue;
                }
                let mask = &mut masks[(slice * N * N) as usize..((slice + 1) * N * N) as usize];
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
                        emit_quad(&mut mesh, &tops, offset, axis, u_ax, v_ax, sign, slice, i, j, w, h, key);
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
fn emit_quad(mesh: &mut Mesh, tops: &[i32], offset: IVec3, axis: usize, u_ax: usize, v_ax: usize, sign: i32, slice: i32, i: i32, j: i32, w: i32, h: i32, key: u32) {
    let m = (key & 0xff) as Mat;
    let ao = (key >> 8) & 0xff;
    let plane = if sign > 0 { slice + 1 } else { slice };
    let grid = |du: i32, dv: i32| -> [i32; 3] {
        let mut c = [0i32; 3];
        c[axis] = plane;
        c[u_ax] = i + du;
        c[v_ax] = j + dv;
        c
    };
    let terrain = is_terrain(m);
    let rgb = palette::color(m);
    let color = [rgb[0], rgb[1], rgb[2], m];
    let dir = dir_index(axis, sign);
    // Esquinas en orden antihorario visto desde la normal +eje (u × v = eje).
    let corners = [(0, 0), (w, 0), (w, h), (0, h)];
    let aos = [ao & 3, (ao >> 2) & 3, (ao >> 4) & 3, (ao >> 6) & 3];
    let base = mesh.vertices.len() as u32;
    for (k, (du, dv)) in corners.iter().enumerate() {
        let g = grid(*du, *dv);
        let (sx, sz) = if terrain {
            let sn = smooth_normal(tops, g[0] + 1, g[2] + 1);
            ((sn[0] as i32 + 128) as u8, (sn[2] as i32 + 128) as u8)
        } else {
            (128, 128)
        };
        let p = [(g[0] + offset.x) as u16, (g[1] + offset.y) as u16, (g[2] + offset.z) as u16];
        for a in 0..3 {
            mesh.lo[a] = mesh.lo[a].min(p[a]);
            mesh.hi[a] = mesh.hi[a].max(p[a]);
        }
        mesh.vertices.push(Vertex { pos: [p[0], p[1], p[2], 0], color, info: [dir as u8 | ((aos[k] as u8) << 3), sx, sz, 0] });
    }
    // Se elige la diagonal según la oclusión para evitar el defecto de interpolación.
    let flip = aos[0] + aos[2] < aos[1] + aos[3];
    let tris: [u32; 6] = if flip { [1, 2, 3, 1, 3, 0] } else { [0, 1, 2, 0, 2, 3] };
    for t in tris {
        let idx = if sign > 0 { t } else { [0, 3, 2, 1][t as usize] };
        mesh.groups[dir].push(base + idx);
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
        let m = mesh_chunk(&v, IVec3::ZERO);
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
        let m = mesh_chunk(&v, IVec3::ZERO);
        // Arriba y abajo: un solo quad cada uno; los 4 bordes: un quad por lado.
        assert_eq!(m.triangles(), 12);
    }

    #[test]
    fn la_normal_suave_sigue_la_ladera() {
        // Terreno en escalera que sube hacia +x: la normal suave debe inclinarse hacia -x.
        let mut v = empty();
        for z in 0..P {
            for x in 0..P {
                for y in 0..=(x / 4 + 2).min(P - 1) {
                    v[(x + z * P + y * P * P) as usize] = palette::GRASS;
                }
            }
        }
        let m = mesh_chunk(&v, IVec3::ZERO);
        let ups: Vec<&Vertex> = m.vertices.iter().filter(|q| q.dir() == 0).collect();
        assert!(!ups.is_empty());
        let avg_x: f32 = ups.iter().map(|q| q.smooth().x).sum::<f32>() / ups.len() as f32;
        assert!(avg_x < -0.08, "la normal suave no se inclina con la pendiente ({avg_x})");
    }

    #[test]
    fn las_normales_apuntan_hacia_afuera() {
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, IVec3::ZERO);
        let (indices, _) = m.indices_grouped();
        for t in indices.chunks(3) {
            let a = m.world_pos(&m.vertices[t[0] as usize]);
            let b = m.world_pos(&m.vertices[t[1] as usize]);
            let c = m.world_pos(&m.vertices[t[2] as usize]);
            let face = (b - a).cross(c - a);
            assert!(face.dot(m.vertices[t[0] as usize].normal()) > 0.0, "triángulo con orden invertido");
        }
    }

    #[test]
    fn el_vertice_compacto_ocupa_16_bytes_y_se_desempaqueta_bien() {
        assert_eq!(std::mem::size_of::<Vertex>(), 16);
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, IVec3::new(0, 64, 0));
        // Cada grupo tiene solo caras de su dirección, y la posición respeta el desplazamiento.
        for (d, g) in m.groups.iter().enumerate() {
            assert_eq!(g.len(), 6, "una cara por dirección");
            for &i in g {
                let q = &m.vertices[i as usize];
                assert_eq!(q.dir(), d);
                assert!(q.pos[1] >= 64 + 4 && q.pos[1] <= 64 + 5);
                assert!(q.ao() <= 3);
            }
        }
        assert_eq!((m.lo, m.hi), ([4, 68, 4], [5, 69, 5]));
    }
}
