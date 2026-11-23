//! [lod] Caché de mallas de columnas en RAM con límite real de bytes (técnica del mod LOD de
//! Minecraft, `storage/BoundedRegionCache` 0.26.6; ver `docs/sistemas/tecnicas-minecraft-lod.md`
//! §2.11).
//!
//! Una columna que sale de los anillos (zoom o paneo) se borra de la GPU, pero su malla queda
//! acá: si la cámara vuelve, se sube de nuevo sin generarla ni mallarla. Una malla depende solo
//! de su nivel y columna, de qué vecinas están en el anillo (los faldones del borde) y de las
//! ediciones cercanas: eso es la [`Firma`]. El terreno es una función fija por `Streamer`. Se
//! guarda por (columna, firma): una columna del borde de un anillo tiene dos mallas (con y sin
//! la vecina de adentro) y en un zoom de ida y vuelta se usan las dos.
//!
//! El límite cuenta **todo** lo que ocupa una entrada: la estructura de la malla, la capacidad
//! de sus vectores y un costo fijo por entrada del mapa y del orden LRU. La lección del mod:
//! sin el costo fijo, la caché ocupaba ~3× su presupuesto.

use crate::mesher::Mesh;
use crate::stream::Key;
use std::collections::BTreeMap;

/// Costo fijo de una entrada (mapa hash + árbol del orden LRU + firma), en bytes.
pub const COSTO_ENTRADA: usize = 96;

/// De qué depende una malla, además de su clave: vecinas presentes en el anillo (8 bits, una
/// por vecina) y la generación de ediciones de la zona.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Firma {
    pub vecinas: u8,
    pub ediciones: u32,
}

/// Bytes que ocupa una malla en RAM (estructura + capacidad de sus vectores).
pub fn bytes_malla(m: &Mesh) -> usize {
    std::mem::size_of::<Mesh>()
        + m.quads.iter().map(|g| g.capacity()).sum::<usize>() * std::mem::size_of::<crate::mesher::Quad>()
        + m.water.capacity() * std::mem::size_of::<crate::mesher::Quad>()
        + m.detail.capacity() * std::mem::size_of::<crate::trees::Micro>()
}

struct Entrada {
    malla: Mesh,
    bytes: usize,
    uso: u64,
}

/// LRU por bytes. La usa solo el hilo principal (el `Streamer`), así que no necesita candados.
pub struct CacheMallas {
    mapa: std::collections::HashMap<(Key, Firma), Entrada, crate::fxhash::FxBuild>,
    orden: BTreeMap<u64, (Key, Firma)>,
    reloj: u64,
    bytes: usize,
    pub presupuesto: usize,
    pub aciertos: usize,
    pub desalojos: usize,
}

impl CacheMallas {
    pub fn new(presupuesto: usize) -> CacheMallas {
        CacheMallas { mapa: Default::default(), orden: BTreeMap::new(), reloj: 0, bytes: 0, presupuesto, aciertos: 0, desalojos: 0 }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.mapa.len()
    }

    fn tick(&mut self) -> u64 {
        self.reloj += 1;
        self.reloj
    }

    /// Guarda una copia de la malla (reemplaza la que hubiera de esa clave) y desaloja lo menos
    /// usado hasta volver a entrar en el presupuesto.
    pub fn guardar(&mut self, key: Key, firma: Firma, malla: &Mesh) {
        self.guardar_propia(key, firma, malla.clone());
    }

    /// Igual que [`guardar`], quedándose con la malla (sin copiarla: el `Streamer` le devuelve
    /// la que se acaba de subir a la GPU).
    pub fn guardar_propia(&mut self, key: Key, firma: Firma, malla: Mesh) {
        if self.presupuesto == 0 {
            return;
        }
        let bytes = bytes_malla(&malla) + COSTO_ENTRADA;
        if bytes > self.presupuesto {
            self.quitar(key, firma);
            return;
        }
        let uso = self.tick();
        if let Some(v) = self.mapa.insert((key, firma), Entrada { malla, bytes, uso }) {
            self.orden.remove(&v.uso);
            self.bytes -= v.bytes;
        }
        self.orden.insert(uso, (key, firma));
        self.bytes += bytes;
        self.recortar();
    }

    /// La malla de la clave, si su firma es la pedida (una copia: la entrada queda).
    pub fn buscar(&mut self, key: Key, firma: Firma) -> Option<Mesh> {
        let uso = self.tick();
        let e = self.mapa.get_mut(&(key, firma))?;
        self.orden.remove(&e.uso);
        e.uso = uso;
        self.orden.insert(uso, (key, firma));
        self.aciertos += 1;
        Some(e.malla.clone())
    }

    /// Saca la malla de la caché (sin copiarla), si está con esa firma.
    pub fn sacar(&mut self, key: Key, firma: Firma) -> Option<Mesh> {
        let e = self.mapa.remove(&(key, firma))?;
        self.orden.remove(&e.uso);
        self.bytes -= e.bytes;
        self.aciertos += 1;
        Some(e.malla)
    }

    /// Marca la entrada como recién usada (una columna que sale de los anillos es la que más
    /// probablemente vuelva: que sea la última en desalojarse).
    pub fn tocar(&mut self, key: Key, firma: Firma) {
        let uso = self.tick();
        if let Some(e) = self.mapa.get_mut(&(key, firma)) {
            self.orden.remove(&e.uso);
            e.uso = uso;
            self.orden.insert(uso, (key, firma));
        }
    }

    pub fn quitar(&mut self, key: Key, firma: Firma) {
        if let Some(e) = self.mapa.remove(&(key, firma)) {
            self.orden.remove(&e.uso);
            self.bytes -= e.bytes;
        }
    }

    pub fn set_presupuesto(&mut self, bytes: usize) {
        if self.presupuesto == 0 && std::env::var("MOTOR_SIN_CACHE").is_ok() {
            return;
        }
        self.presupuesto = bytes;
        self.recortar();
    }

    pub fn vaciar(&mut self) {
        self.mapa.clear();
        self.orden.clear();
        self.bytes = 0;
    }

    fn recortar(&mut self) {
        while self.bytes > self.presupuesto {
            let Some((&uso, &key)) = self.orden.iter().next() else { break };
            // `key` es (columna, firma).
            self.orden.remove(&uso);
            if let Some(e) = self.mapa.remove(&key) {
                self.bytes -= e.bytes;
            }
            self.desalojos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesher::Quad;
    use glam::{IVec2, Vec3};

    fn malla(caras: usize) -> Mesh {
        let mut m = Mesh::new(Vec3::ZERO, 1.0);
        m.quads[2] = vec![Quad { a: 1, b: 2, smooth: [0; 2], slot: 0 }; caras];
        m
    }

    #[test]
    fn el_limite_de_bytes_se_cumple_contando_el_costo_fijo() {
        let una = bytes_malla(&malla(100)) + COSTO_ENTRADA;
        let mut c = CacheMallas::new(una * 10);
        for k in 0..50 {
            c.guardar((0, IVec2::new(k, 0)), Firma::default(), &malla(100));
            assert!(c.bytes() <= c.presupuesto, "{} bytes con presupuesto {}", c.bytes(), c.presupuesto);
        }
        assert_eq!(c.len(), 10);
        assert_eq!(c.desalojos, 40);
        // Lo que queda es lo último guardado.
        assert!(c.buscar((0, IVec2::new(49, 0)), Firma::default()).is_some());
        assert!(c.buscar((0, IVec2::new(0, 0)), Firma::default()).is_none());
        // El costo fijo cuenta: con mallas vacías tampoco se pasa (el error del mod: ~3× lo previsto).
        let mut c = CacheMallas::new(10_000);
        for k in 0..1000 {
            c.guardar((1, IVec2::new(k, k)), Firma::default(), &Mesh::default());
        }
        assert!(c.bytes() <= 10_000);
        assert_eq!(c.len(), 10_000 / (bytes_malla(&Mesh::default()) + COSTO_ENTRADA));
    }

    #[test]
    fn una_firma_distinta_no_sirve_y_lo_tocado_se_desaloja_ultimo() {
        let una = bytes_malla(&malla(10)) + COSTO_ENTRADA;
        let mut c = CacheMallas::new(una * 3);
        let f = Firma { vecinas: 0xff, ediciones: 3 };
        for k in 0..3 {
            c.guardar((0, IVec2::new(k, 0)), f, &malla(10));
        }
        assert!(c.buscar((0, IVec2::new(1, 0)), Firma { vecinas: 0x7f, ediciones: 3 }).is_none());
        assert!(c.buscar((0, IVec2::new(1, 0)), Firma { vecinas: 0xff, ediciones: 4 }).is_none());
        assert_eq!(c.aciertos, 0);
        // La 0 es la más vieja, pero se tocó: se desaloja la 1 (la 2 se buscó recién).
        c.buscar((0, IVec2::new(2, 0)), f).unwrap();
        c.tocar((0, IVec2::new(0, 0)), f);
        c.guardar((0, IVec2::new(3, 0)), f, &malla(10));
        assert!(c.buscar((0, IVec2::new(1, 0)), f).is_none());
        assert_eq!(c.buscar((0, IVec2::new(0, 0)), f).unwrap().quad_count(), 10);
        // Dos firmas de la misma columna conviven (un borde de anillo, con y sin la vecina).
        let g = Firma { vecinas: 0x0f, ediciones: 3 };
        c.guardar((0, IVec2::new(0, 0)), g, &malla(10));
        assert!(c.buscar((0, IVec2::new(0, 0)), g).is_some() && c.buscar((0, IVec2::new(0, 0)), f).is_some());
        // Achicar el presupuesto desaloja enseguida.
        c.set_presupuesto(una);
        assert_eq!(c.len(), 1);
    }
}
