//! Un chunk: 32 × 32 × 32 vóxeles de un material cada uno.

use crate::lod::CHUNK;
use crate::palette::{Mat, AIR};

const N: usize = CHUNK as usize;

pub struct Chunk {
    data: Box<[Mat]>,
    solid: u32,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk { data: vec![AIR; N * N * N].into_boxed_slice(), solid: 0 }
    }

    #[inline]
    pub fn index(x: usize, y: usize, z: usize) -> usize {
        x + z * N + y * N * N
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Mat {
        self.data[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, m: Mat) {
        let i = Self::index(x, y, z);
        let old = self.data[i];
        if old == AIR && m != AIR {
            self.solid += 1;
        } else if old != AIR && m == AIR {
            self.solid -= 1;
        }
        self.data[i] = m;
    }

    pub fn is_empty(&self) -> bool {
        self.solid == 0
    }

    pub fn is_full(&self) -> bool {
        self.solid as usize == N * N * N
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}
