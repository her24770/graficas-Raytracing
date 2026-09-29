use nalgebra_glm::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct Material {
    pub diffuse: f32,
    pub specular: f32,
    pub specular_exponent: f32,
    pub reflectivity: f32,
    pub transparency: f32,
    pub refractive_index: f32,
    pub emission: f32,
    // Si es true, la textura se desliza con el tiempo (solo lo usa el agua por ahora).
    pub animated: bool,
}

impl Material {
    pub const fn matte(specular: f32, specular_exponent: f32) -> Self {
        Material {
            diffuse: 0.9,
            specular,
            specular_exponent,
            reflectivity: 0.0,
            transparency: 0.0,
            refractive_index: 1.0,
            emission: 0.0,
            animated: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FaceTextures {
    pub top: usize,
    pub side: usize,
    pub bottom: usize,
}

impl FaceTextures {
    pub fn uniform(texture_id: usize) -> Self {
        FaceTextures {
            top: texture_id,
            side: texture_id,
            bottom: texture_id,
        }
    }

    pub fn top_side_bottom(top: usize, side: usize, bottom: usize) -> Self {
        FaceTextures { top, side, bottom }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Intersect {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub u: f32,
    pub v: f32,
    pub texture_id: usize,
    pub ambient_occlusion: f32,
    pub material: Material,
}

pub trait RayIntersect {
    fn ray_intersect(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<Intersect>;
}
