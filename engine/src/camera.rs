//! Cámara de constructor de ciudades: orbita alrededor de un punto del suelo.

use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Giro alrededor del eje vertical, en grados (0 = mirando hacia el norte, −z).
    pub yaw: f32,
    /// Inclinación hacia abajo, en grados (positivo = mira desde arriba).
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        // El ojo queda "detrás" del objetivo respecto a la dirección de la mirada.
        let back = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
        self.target + back * self.distance
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    /// Proyección con Z invertida e infinita: buena precisión de profundidad de 0,5 m a 40 km.
    pub fn proj(&self, aspect: f32) -> Mat4 {
        let near = (self.distance * 0.002).clamp(0.1, 20.0);
        Mat4::perspective_infinite_reverse_rh(self.fov_y.to_radians(), aspect, near)
    }

    pub fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(5.0, 89.0);
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(10.0, 30000.0);
    }
}
