use crate::color::Color;
use nalgebra_glm::Vec3;

pub struct Light {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
    // Distancia a la que la luz se apaga por completo; infinito para el sol.
    pub range: f32,
}

impl Light {
    pub fn new(position: Vec3, color: Color, intensity: f32, range: f32) -> Self {
        Light {
            position,
            color,
            intensity,
            range,
        }
    }

    pub fn attenuation(&self, distance: f32) -> f32 {
        if self.range.is_infinite() {
            return 1.0;
        }
        let falloff = (1.0 - distance / self.range).max(0.0);
        falloff * falloff
    }
}
