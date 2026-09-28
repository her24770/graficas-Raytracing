use nalgebra_glm::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct Material {
    pub diffuse: f32,
    pub specular: f32,
    pub reflectivity: f32,
    pub transparency: f32,
    pub refractive_index: f32,
    pub emission: f32,
}

impl Material {
    pub fn new(
        diffuse: f32,
        specular: f32,
        reflectivity: f32,
        transparency: f32,
        refractive_index: f32,
        emission: f32,
    ) -> Self {
        Material {
            diffuse,
            specular,
            reflectivity,
            transparency,
            refractive_index,
            emission,
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
    pub material: Material,
}

pub trait RayIntersect {
    fn ray_intersect(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<Intersect>;
}
