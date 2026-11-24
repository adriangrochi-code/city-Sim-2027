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

/// Cara voraz compacta (20 bytes): reemplaza 4 vértices de 16 bytes + 6 índices (76 bytes).
/// El vertex shader la expande en 2 triángulos con `vertex_index` (*vertex pulling*).
///
/// Idea tomada de Tisma engine (`apps/tisma/shaders/preview.vert` + `PreviewInstance`): cada
/// primitiva es una instancia mínima y el shader arma la geometría. Aquí la instancia es la
/// cara voraz; además lleva las normales suaves de sus 4 esquinas (terreno) y el número de
/// columna (para encontrar su origen en el dibujo indirecto múltiple).
///
/// - `a`: x (6 bits) | z (6) << 6 | y (11) << 12 | dirección (3) << 23. Es la esquina (0, 0)
///   de la cara, en vóxeles relativos al origen de la malla (incluye el plano de la cara).
/// - `b`: ancho − 1 (5) | alto − 1 (5) << 5 | oclusión de las 4 esquinas (8) << 10 | material (8) << 18.
/// - `smooth`: normal suave (x, z) + 128 de las esquinas 0..3, un byte cada una.
/// - `slot`: columna (lo completa el renderer al subirla).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
pub struct Quad {
    pub a: u32,
    pub b: u32,
    pub smooth: [u32; 2],
    pub slot: u32,
}

/// Límite de la coordenada y de una malla (11 bits).
pub const MAX_Y: i32 = 2047;

#[cfg_attr(not(test), allow(dead_code))]
impl Quad {
    pub fn dir(&self) -> usize {
        ((self.a >> 23) & 7) as usize
    }
    pub fn base(&self) -> [i32; 3] {
        [(self.a & 63) as i32, ((self.a >> 12) & 2047) as i32, ((self.a >> 6) & 63) as i32]
    }
    pub fn size(&self) -> (i32, i32) {
        ((self.b & 31) as i32 + 1, ((self.b >> 5) & 31) as i32 + 1)
    }
    pub fn ao(&self) -> u32 {
        (self.b >> 10) & 0xff
    }
    pub fn material(&self) -> Mat {
        ((self.b >> 18) & 0xff) as Mat
    }
    fn smooth_at(&self, k: usize) -> (u8, u8) {
        let w = self.smooth[k / 2] >> ((k % 2) * 16);
        ((w & 0xff) as u8, ((w >> 8) & 0xff) as u8)
    }
    /// Los 4 vértices y los 6 índices (0..4) de la cara, igual que el vertex shader.
    pub fn expand(&self) -> ([Vertex; 4], [u32; 6]) {
        let dir = self.dir();
        let d = DIRS[dir];
        let axis = if d[0] != 0 { 0 } else if d[1] != 0 { 1 } else { 2 };
        let sign = d[0] + d[1] + d[2];
        let (u_ax, v_ax) = ((axis + 1) % 3, (axis + 2) % 3);
        let (w, h) = self.size();
        let base = self.base();
        let ao = self.ao();
        let aos = [ao & 3, (ao >> 2) & 3, (ao >> 4) & 3, (ao >> 6) & 3];
        let m = self.material();
        let rgb = palette::color(m);
        let mut verts = [Vertex::zeroed(); 4];
        for (k, (du, dv)) in [(0, 0), (w, 0), (w, h), (0, h)].iter().enumerate() {
            let mut p = base;
            p[u_ax] += du;
            p[v_ax] += dv;
            let (sx, sz) = self.smooth_at(k);
            verts[k] = Vertex { pos: [p[0] as u16, p[1] as u16, p[2] as u16, 0], color: [rgb[0], rgb[1], rgb[2], m], info: [dir as u8 | ((aos[k] as u8) << 3), sx, sz, 0] };
        }
        let flip = aos[0] + aos[2] < aos[1] + aos[3];
        let tris: [u32; 6] = if flip { [1, 2, 3, 1, 3, 0] } else { [0, 1, 2, 0, 2, 3] };
        let mut idx = [0u32; 6];
        for (k, t) in tris.iter().enumerate() {
            idx[k] = if sign > 0 { *t } else { [0, 3, 2, 1][*t as usize] };
        }
        (verts, idx)
    }
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
    matches!(m, GRASS | DIRT | SAND | ROCK | MUD | COBBLE | DIRT_ROAD | PLAZA | GRASS_DRY | GARDEN | SAND_WET | MARSH | GRASS_LUSH | COAST_ROCK | DUNE | STREAM | FOREST_FLOOR)
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

/// Terreno natural (no calles, plaza ni huertas, que están aplanadas por obras).
fn is_natural(m: Mat) -> bool {
    use palette::*;
    !matches!(m, COBBLE | DIRT_ROAD | PLAZA | GARDEN)
}

/// Normal en una esquina a partir de la pendiente continua de las cuatro columnas que la tocan.
fn grad_normal(grad: &[f32], cx: i32, cz: i32) -> [i8; 4] {
    let (mut gx, mut gz) = (0.0, 0.0);
    for (x, z) in [(cx - 1, cz - 1), (cx, cz - 1), (cx - 1, cz), (cx, cz)] {
        let k = (x.clamp(0, P - 1) + z.clamp(0, P - 1) * P) as usize * 2;
        gx += grad[k];
        gz += grad[k + 1];
    }
    let n = Vec3::new(-gx / 4.0, 1.0, -gz / 4.0).normalize();
    [(n.x * 127.0) as i8, (n.y * 127.0) as i8, (n.z * 127.0) as i8, 0]
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

/// Malla de una columna (o de un chunk): caras compactas agrupadas por dirección
/// ([`DIRS`]), con posición relativa a `origin`, y caja envolvente.
#[derive(Clone)] // [lod] la caché de columnas guarda copias (ver `cache.rs`)
pub struct Mesh {
    pub quads: [Vec<Quad>; 6],
    /// Origen de la malla en metros y tamaño del vóxel.
    pub origin: Vec3,
    pub voxel: f32,
    /// Caja envolvente en vóxeles relativos al origen (min, max).
    pub lo: [u16; 3],
    pub hi: [u16; 3],
    /// Flora baja en micro-vóxeles (solo en el nivel más fino), dibujada aparte.
    pub detail: Vec<crate::trees::Micro>,
    /// [agua en vóxeles] Caras del agua (ver `agua::mallar`), dibujadas en el pase del agua.
    pub water: Vec<Quad>,
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh { quads: Default::default(), origin: Vec3::ZERO, voxel: 1.0, lo: [u16::MAX; 3], hi: [0; 3], detail: Vec::new(), water: Vec::new() }
    }
}

impl Mesh {
    pub fn new(origin: Vec3, voxel: f32) -> Mesh {
        Mesh { origin, voxel, ..Default::default() }
    }
    pub fn quad_count(&self) -> usize {
        self.quads.iter().map(|g| g.len()).sum::<usize>() + self.water.len()
    }
    pub fn triangles(&self) -> usize {
        self.quad_count() * 2
    }
    pub fn is_empty(&self) -> bool {
        self.quad_count() == 0
    }
    /// Une otra malla con el mismo origen.
    pub fn append(&mut self, other: &Mesh) {
        for (g, og) in self.quads.iter_mut().zip(other.quads.iter()) {
            g.extend_from_slice(og);
        }
        for k in 0..3 {
            self.lo[k] = self.lo[k].min(other.lo[k]);
            self.hi[k] = self.hi[k].max(other.hi[k]);
        }
    }
    /// Todas las caras seguidas y dónde empieza cada grupo (7 valores).
    pub fn quads_grouped(&self) -> (Vec<Quad>, [u32; 7]) {
        let mut out = Vec::with_capacity(self.quad_count());
        let mut starts = [0u32; 7];
        for (k, g) in self.quads.iter().enumerate() {
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
    /// Expande las caras a vértices e índices agrupados por dirección (para pruebas: es lo
    /// mismo que hace el vertex shader).
    #[cfg(test)]
    pub fn expanded(&self) -> (Vec<Vertex>, [Vec<u32>; 6]) {
        let mut verts = Vec::new();
        let mut groups: [Vec<u32>; 6] = Default::default();
        for (d, g) in self.quads.iter().enumerate() {
            for q in g {
                let (v, idx) = q.expand();
                let base = verts.len() as u32;
                verts.extend_from_slice(&v);
                groups[d].extend(idx.iter().map(|i| i + base));
            }
        }
        (verts, groups)
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
    mesh_chunk_grad(v, offset, None)
}

/// Igual que [`mesh_chunk`], con la pendiente continua del terreno de cada columna de la
/// copia con borde (`grad[(x + z * P) * 2 ..]` = dh/dx, dh/dz). Con ella, la normal suave de
/// las laderas sale del relieve real y no de los escalones enteros: en una pendiente suave,
/// los escalones daban una normal que saltaba de plana a empinada en cada borde y la ladera
/// se veía rayada como un mapa de curvas de nivel.
pub fn mesh_chunk_grad(v: &[Mat], offset: IVec3, grad: Option<&[f32]>) -> Mesh {
    mesh_chunk_ao(v, offset, grad, AoModo::Todo)
}

/// [render] Dónde se calcula la oclusión por vértice (técnicas del mod LOD §2.14). La oclusión
/// es parte de la clave del *greedy*: dos caras vecinas con distinta oclusión no se unen. En los
/// niveles gruesos el shader casi no la muestra en el terreno (ver `ao_min` en voxel.wgsl), así
/// que ahí se pagaban caras que no se ven.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AoModo {
    Todo,
    /// Sin oclusión en el terreno (sí en las obras y los árboles).
    SinTerreno,
    /// Sin oclusión en nada.
    Nada,
}

impl AoModo {
    /// El modo de cada nivel: terreno sin oclusión desde el vóxel de 16 m (nivel 6), nada desde
    /// el de 64 m (nivel 8, el entorno amplio). Más cerca no: cada anillo tiene vóxeles de 2 a
    /// 4 píxeles, y en su borde de adentro el shader todavía muestra la oclusión (se probó desde
    /// el nivel 4: el campo del Cerro se veía más claro y plano, 19 % de píxeles cambiados).
    pub fn para_nivel(lod: u32) -> AoModo {
        match lod {
            0..=5 => AoModo::Todo,
            6 | 7 => AoModo::SinTerreno,
            _ => AoModo::Nada,
        }
    }
}

pub fn mesh_chunk_ao(v: &[Mat], offset: IVec3, grad: Option<&[f32]>, ao_modo: AoModo) -> Mesh {
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
                        let ao = match ao_modo {
                            AoModo::Nada => 0xff,
                            AoModo::SinTerreno if is_terrain(m) => 0xff,
                            _ => face_ao(v, [x + n[0], y + n[1], z + n[2]], u_ax, v_ax),
                        };
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
                        emit_quad(&mut mesh, &tops, grad, offset, axis, u_ax, v_ax, sign, slice, i, j, w, h, key);
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
fn emit_quad(mesh: &mut Mesh, tops: &[i32], grad: Option<&[f32]>, offset: IVec3, axis: usize, u_ax: usize, v_ax: usize, sign: i32, slice: i32, i: i32, j: i32, w: i32, h: i32, key: u32) {
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
    let dir = dir_index(axis, sign);
    let mut smooth = [0u32; 2];
    for (k, (du, dv)) in [(0, 0), (w, 0), (w, h), (0, h)].iter().enumerate() {
        let g = grid(*du, *dv);
        let (sx, sz) = if terrain {
            let sn = match grad {
                Some(gr) if is_natural(m) => grad_normal(gr, g[0] + 1, g[2] + 1),
                _ => smooth_normal(tops, g[0] + 1, g[2] + 1),
            };
            ((sn[0] as i32 + 128) as u32, (sn[2] as i32 + 128) as u32)
        } else {
            (128, 128)
        };
        smooth[k / 2] |= (sx | (sz << 8)) << ((k % 2) * 16);
        for a in 0..3 {
            let p = (g[a] + offset[a]) as u16;
            mesh.lo[a] = mesh.lo[a].min(p);
            mesh.hi[a] = mesh.hi[a].max(p);
        }
    }
    let b0 = grid(0, 0);
    let (x, y, z) = (b0[0] + offset.x, b0[1] + offset.y, b0[2] + offset.z);
    debug_assert!((0..64).contains(&x) && (0..64).contains(&z) && (0..=MAX_Y).contains(&y), "cara fuera de rango: {x} {y} {z}");
    let qa = (x as u32 & 63) | ((z as u32 & 63) << 6) | ((y.min(MAX_Y) as u32) << 12) | ((dir as u32) << 23);
    let qb = (w as u32 - 1) | ((h as u32 - 1) << 5) | (ao << 10) | ((m as u32) << 18);
    mesh.quads[dir].push(Quad { a: qa, b: qb, smooth, slot: 0 });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Malla con vértices e índices, como antes del formato de caras compactas.
    #[derive(Default)]
    struct LegacyMesh {
        vertices: Vec<Vertex>,
        groups: [Vec<u32>; 6],
    }

    #[allow(clippy::too_many_arguments)]
    fn legacy_emit(mesh: &mut LegacyMesh, tops: &[i32], offset: IVec3, axis: usize, u_ax: usize, v_ax: usize, sign: i32, slice: i32, i: i32, j: i32, w: i32, h: i32, key: u32) {
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
                let _ = a;
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


    /// Malla "como antes": las mismas caras voraces emitidas con el emisor anterior.
    fn legacy_from(v: &[Mat], m: &Mesh) -> LegacyMesh {
        let tops = terrain_tops(v);
        let mut out = LegacyMesh::default();
        for g in &m.quads {
            for q in g {
                let d = DIRS[q.dir()];
                let axis = if d[0] != 0 { 0 } else if d[1] != 0 { 1 } else { 2 };
                let sign = d[0] + d[1] + d[2];
                let (u_ax, v_ax) = ((axis + 1) % 3, (axis + 2) % 3);
                let base = q.base();
                let plane = base[axis];
                let slice = if sign > 0 { plane - 1 } else { plane };
                let (w, h) = q.size();
                let key = q.material() as u32 | (q.ao() << 8);
                legacy_emit(&mut out, &tops, IVec3::ZERO, axis, u_ax, v_ax, sign, slice, base[u_ax], base[v_ax], w, h, key);
            }
        }
        out
    }

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
        let (verts, _) = m.expanded();
        let ups: Vec<&Vertex> = verts.iter().filter(|q| q.dir() == 0).collect();
        assert!(!ups.is_empty());
        let avg_x: f32 = ups.iter().map(|q| q.smooth().x).sum::<f32>() / ups.len() as f32;
        assert!(avg_x < -0.08, "la normal suave no se inclina con la pendiente ({avg_x})");
    }

    /// [render] Sin oclusión en el terreno grueso: las caras de una escalera se unen más (la
    /// oclusión de los escalones cortaba las uniones) y ninguna lleva oclusión; una obra sobre
    /// el terreno la conserva salvo con `AoModo::Nada`.
    #[test]
    fn sin_oclusion_en_el_terreno_hay_menos_caras() {
        let mut v = empty();
        for z in 0..P {
            for x in 0..P {
                for y in 0..=((x + z) / 3 + 2).min(P - 1) {
                    v[(x + z * P + y * P * P) as usize] = palette::GRASS;
                }
            }
        }
        v[(10 + 10 * P + 9 * P * P) as usize] = palette::CAL;
        let todo = mesh_chunk_ao(&v, IVec3::ZERO, None, AoModo::Todo);
        let sin = mesh_chunk_ao(&v, IVec3::ZERO, None, AoModo::SinTerreno);
        let nada = mesh_chunk_ao(&v, IVec3::ZERO, None, AoModo::Nada);
        assert!(sin.quad_count() < todo.quad_count(), "{} contra {}", sin.quad_count(), todo.quad_count());
        assert!(nada.quad_count() <= sin.quad_count());
        let all: Vec<&Quad> = sin.quads.iter().flatten().collect();
        assert!(all.iter().filter(|q| is_terrain(q.material())).all(|q| q.ao() == 0xff));
        assert!(all.iter().any(|q| q.material() == palette::CAL && q.ao() != 0xff), "la obra conserva su oclusión");
        assert!(nada.quads.iter().flatten().all(|q| q.ao() == 0xff));
        assert_eq!(AoModo::para_nivel(5), AoModo::Todo);
    }

    #[test]
    fn las_normales_apuntan_hacia_afuera() {
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, IVec3::ZERO);
        let (verts, groups) = m.expanded();
        for g in &groups {
            for t in g.chunks(3) {
                let a = m.world_pos(&verts[t[0] as usize]);
                let b = m.world_pos(&verts[t[1] as usize]);
                let c = m.world_pos(&verts[t[2] as usize]);
                let face = (b - a).cross(c - a);
                assert!(face.dot(verts[t[0] as usize].normal()) > 0.0, "triángulo con orden invertido");
            }
        }
    }

    #[test]
    fn la_cara_compacta_ocupa_20_bytes_y_se_desempaqueta_bien() {
        assert_eq!(std::mem::size_of::<Vertex>(), 16);
        assert_eq!(std::mem::size_of::<Quad>(), 20);
        let mut v = empty();
        v[(5 + 5 * P + 5 * P * P) as usize] = palette::CAL;
        let m = mesh_chunk(&v, IVec3::new(0, 64, 0));
        // Cada grupo tiene solo caras de su dirección, y la posición respeta el desplazamiento.
        let (verts, groups) = m.expanded();
        for (d, g) in groups.iter().enumerate() {
            assert_eq!(g.len(), 6, "una cara por dirección");
            for &i in g {
                let q = &verts[i as usize];
                assert_eq!(q.dir(), d);
                assert!(q.pos[1] >= 64 + 4 && q.pos[1] <= 64 + 5);
                assert!(q.ao() <= 3);
                assert_eq!(q.color[3], palette::CAL);
            }
        }
        assert_eq!((m.lo, m.hi), ([4, 68, 4], [5, 69, 5]));
    }

    /// Un terreno irregular con casas: la expansión de las caras compactas (lo que hace el
    /// vertex shader) debe dar exactamente los mismos vértices e índices que el emisor anterior.
    #[test]
    fn la_expansion_de_las_caras_es_igual_a_la_malla_anterior() {
        let mut v = empty();
        for z in 0..P {
            for x in 0..P {
                let h = 3 + ((x * 7 + z * 13) % 5) + x / 6;
                for y in 0..=h.min(P - 1) {
                    v[(x + z * P + y * P * P) as usize] = if y == h { palette::GRASS } else { palette::DIRT };
                }
            }
        }
        for y in 10..18 {
            for z in 8..14 {
                for x in 8..20 {
                    v[(x + z * P + y * P * P) as usize] = if y == 17 { palette::TILE } else { palette::CAL };
                }
            }
        }
        let m = mesh_chunk(&v, IVec3::ZERO);
        let legacy = legacy_from(&v, &m);
        let (verts, groups) = m.expanded();
        assert!(m.quad_count() > 100);
        assert_eq!(verts.len(), legacy.vertices.len());
        for (a, b) in verts.iter().zip(legacy.vertices.iter()) {
            assert_eq!((a.pos, a.color, a.info), (b.pos, b.color, b.info));
        }
        assert_eq!(groups, legacy.groups);
        // Bytes: 20 por cara contra 4 vértices de 16 + 6 índices de 2.
        assert!(m.quad_count() * 20 * 3 < legacy.vertices.len() * 16 + groups.iter().map(|g| g.len()).sum::<usize>() * 2);
    }
}
