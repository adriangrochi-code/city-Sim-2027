//! Asignador por bloques (*buddy*) para el megabúfer de caras de la GPU.
//!
//! Idea tomada de Tisma engine (`apps/tisma/vk_view.cpp`, búferes de datos editables): en vez
//! de crear y destruir un búfer por malla, todas las caras viven en un búfer grande; las
//! mallas piden bloques y los devuelven. Los bloques liberados no se reutilizan hasta que
//! pasaron `frames_in_flight` cuadros (liberación diferida), así nunca hace falta esperar a
//! que la GPU termine todo (`vkDeviceWaitIdle`), que es lo que Tisma todavía hace al re-subir.
//!
//! Los tamaños se redondean a potencias de dos de la unidad; dos bloques "hermanos" libres se
//! vuelven a unir. Las listas libres están ordenadas: con la misma secuencia de pedidos, las
//! direcciones son siempre las mismas (determinismo).

use std::collections::BTreeSet;

#[derive(Debug)]
pub struct Buddy {
    /// Orden del bloque raíz: la capacidad es `1 << max_order` unidades.
    max_order: u32,
    free: Vec<BTreeSet<u32>>,
    /// Bloques liberados que esperan a que la GPU deje de usarlos: (cuadro, offset, orden).
    pending: Vec<(u64, u32, u32)>,
    used_units: u64,
}

pub fn order_for(units: u32) -> u32 {
    units.max(1).next_power_of_two().trailing_zeros()
}

impl Buddy {
    pub fn new(max_order: u32) -> Buddy {
        let mut free = vec![BTreeSet::new(); max_order as usize + 1];
        free[max_order as usize].insert(0);
        Buddy { max_order, free, pending: Vec::new(), used_units: 0 }
    }

    pub fn capacity(&self) -> u64 {
        1u64 << self.max_order
    }

    pub fn used(&self) -> u64 {
        self.used_units
    }

    /// Reserva un bloque de al menos `units` unidades. Devuelve (offset, orden).
    pub fn alloc(&mut self, units: u32) -> Option<(u32, u32)> {
        let order = order_for(units);
        if order > self.max_order {
            return None;
        }
        let mut k = order;
        while k <= self.max_order && self.free[k as usize].is_empty() {
            k += 1;
        }
        if k > self.max_order {
            return None;
        }
        let off = *self.free[k as usize].iter().next().unwrap();
        self.free[k as usize].remove(&off);
        // Se parte el bloque hasta el tamaño pedido; las mitades altas quedan libres.
        while k > order {
            k -= 1;
            self.free[k as usize].insert(off + (1 << k));
        }
        self.used_units += 1 << order;
        Some((off, order))
    }

    /// Devuelve un bloque cuando la GPU ya terminó el cuadro `frame` (liberación diferida).
    pub fn free_later(&mut self, frame: u64, off: u32, order: u32) {
        self.pending.push((frame, off, order));
    }

    /// Libera los bloques cuya espera ya pasó (`now` = cuadro actual).
    pub fn collect(&mut self, now: u64, frames_in_flight: u64) -> usize {
        let mut n = 0;
        let mut keep = Vec::with_capacity(self.pending.len());
        for p in std::mem::take(&mut self.pending) {
            if p.0 + frames_in_flight <= now {
                self.free_now(p.1, p.2);
                n += 1;
            } else {
                keep.push(p);
            }
        }
        self.pending = keep;
        n
    }

    fn free_now(&mut self, mut off: u32, mut order: u32) {
        self.used_units -= 1 << order;
        while order < self.max_order {
            let buddy = off ^ (1 << order);
            if !self.free[order as usize].remove(&buddy) {
                break;
            }
            off = off.min(buddy);
            order += 1;
        }
        self.free[order as usize].insert(off);
    }

    /// Duplica la capacidad (el búfer de la GPU se agranda y se copia lo anterior).
    pub fn grow(&mut self) {
        let old = self.max_order;
        self.free.push(BTreeSet::new());
        self.max_order += 1;
        // Si toda la capacidad anterior estaba libre, se une con la mitad nueva.
        if self.free[old as usize].remove(&0) {
            self.free[self.max_order as usize].insert(0);
        } else {
            self.free[old as usize].insert(1 << old);
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserva_y_libera_uniendo_hermanos() {
        let mut b = Buddy::new(4); // 16 unidades
        let a = b.alloc(3).unwrap(); // 4 unidades
        let c = b.alloc(1).unwrap();
        let d = b.alloc(8).unwrap();
        assert_eq!(a, (0, 2));
        assert_eq!(c, (4, 0));
        assert_eq!(d, (8, 3));
        assert_eq!(b.used(), 4 + 1 + 8);
        // No se superponen.
        let spans = [(a.0, 1 << a.1), (c.0, 1 << c.1), (d.0, 1 << d.1)];
        for (i, x) in spans.iter().enumerate() {
            for y in spans.iter().skip(i + 1) {
                assert!(x.0 + x.1 <= y.0 || y.0 + y.1 <= x.0);
            }
        }
        // La liberación diferida no devuelve nada hasta que pasan los cuadros en vuelo.
        b.free_later(10, a.0, a.1);
        b.free_later(10, c.0, c.1);
        b.free_later(10, d.0, d.1);
        assert_eq!(b.collect(11, 2), 0);
        assert_eq!(b.collect(12, 2), 3);
        assert_eq!(b.used(), 0);
        // Todo volvió a unirse en un solo bloque raíz.
        assert_eq!(b.alloc(16), Some((0, 4)));
    }

    #[test]
    fn crece_cuando_se_llena() {
        let mut b = Buddy::new(3);
        let x = b.alloc(8).unwrap();
        assert!(b.alloc(1).is_none());
        b.grow();
        assert_eq!(b.capacity(), 16);
        let y = b.alloc(8).unwrap();
        assert_eq!(y, (8, 3));
        b.free_later(0, x.0, x.1);
        b.free_later(0, y.0, y.1);
        b.collect(5, 2);
        assert_eq!(b.alloc(16), Some((0, 4)));
    }

    #[test]
    fn es_determinista() {
        let run = || {
            let mut b = Buddy::new(10);
            let mut out = Vec::new();
            for i in 0..200u32 {
                let r = b.alloc(1 + (i * 37) % 50).unwrap();
                out.push(r);
                if i % 3 == 0 {
                    b.free_later(i as u64, r.0, r.1);
                }
                b.collect(i as u64, 2);
            }
            out
        };
        assert_eq!(run(), run());
    }
}
