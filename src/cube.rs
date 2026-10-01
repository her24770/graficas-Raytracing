use crate::ray_intersect::{FaceTextures, Intersect, Material, RayIntersect};
use nalgebra_glm::Vec3;

const EPSILON: f32 = 1e-4;

pub struct Cube {
    pub min: Vec3,
    pub max: Vec3,
    pub material: Material,
    pub textures: FaceTextures,
    // Oclusion ambiental por cara (indice eje*2 + lado positivo) y por esquina
    // (indice i + 2j sobre los otros dos ejes en orden ascendente).
    pub ambient_occlusion: [[f32; 4]; 6],
    // false para lo que se puede atravesar caminando: agua, lava, plantas, particulas.
    pub solid: bool,
}

pub fn face_index(axis: usize, positive: bool) -> usize {
    axis * 2 + positive as usize
}

pub fn face_tangent_axes(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

impl Cube {
    pub fn new(center: Vec3, size: Vec3, material: Material, textures: FaceTextures) -> Self {
        let half = size * 0.5;
        Cube {
            min: center - half,
            max: center + half,
            material,
            textures,
            ambient_occlusion: [[1.0; 4]; 6],
            solid: true,
        }
    }

    fn occlusion_at(&self, point: &Vec3, axis: usize, sign: f32) -> f32 {
        let corners = self.ambient_occlusion[face_index(axis, sign > 0.0)];
        let (b, c) = face_tangent_axes(axis);
        let s = ((point[b] - self.min[b]) / (self.max[b] - self.min[b])).clamp(0.0, 1.0);
        let t = ((point[c] - self.min[c]) / (self.max[c] - self.min[c])).clamp(0.0, 1.0);

        let low = corners[0] + (corners[1] - corners[0]) * s;
        let high = corners[2] + (corners[3] - corners[2]) * s;
        low + (high - low) * t
    }

    fn face_uv(&self, point: &Vec3, axis: usize, sign: f32) -> (f32, f32) {
        let size = self.max - self.min;
        let local = point - self.min;

        match axis {
            0 => {
                let u = local.z / size.z;
                let v = 1.0 - local.y / size.y;
                if sign > 0.0 { (1.0 - u, v) } else { (u, v) }
            }
            1 => {
                let u = local.x / size.x;
                let v = local.z / size.z;
                (u, v)
            }
            _ => {
                let u = local.x / size.x;
                let v = 1.0 - local.y / size.y;
                if sign > 0.0 { (u, v) } else { (1.0 - u, v) }
            }
        }
    }
}

impl RayIntersect for Cube {
    fn ray_intersect(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<Intersect> {
        let mut t_near = f32::NEG_INFINITY;
        let mut t_far = f32::INFINITY;
        let mut near_axis = 0usize;
        let mut near_sign = -1.0f32;
        let mut far_axis = 0usize;
        let mut far_sign = 1.0f32;

        for axis in 0..3 {
            let origin = ray_origin[axis];
            let dir = ray_direction[axis];
            let min = self.min[axis];
            let max = self.max[axis];

            if dir.abs() < EPSILON {
                if origin < min || origin > max {
                    return None;
                }
                continue;
            }

            let inv_dir = 1.0 / dir;
            let (t1, t2, sign1, sign2) = if inv_dir >= 0.0 {
                ((min - origin) * inv_dir, (max - origin) * inv_dir, -1.0, 1.0)
            } else {
                ((max - origin) * inv_dir, (min - origin) * inv_dir, 1.0, -1.0)
            };

            if t1 > t_near {
                t_near = t1;
                near_axis = axis;
                near_sign = sign1;
            }

            if t2 < t_far {
                t_far = t2;
                far_axis = axis;
                far_sign = sign2;
            }

            if t_near > t_far {
                return None;
            }
        }

        let (distance, axis, sign) = if t_near > EPSILON {
            (t_near, near_axis, near_sign)
        } else if t_far > EPSILON {
            (t_far, far_axis, far_sign)
        } else {
            return None;
        };

        let point = ray_origin + ray_direction * distance;

        let mut normal = Vec3::zeros();
        normal[axis] = sign;

        let (u, v) = self.face_uv(&point, axis, sign);

        let texture_id = match axis {
            1 if sign > 0.0 => self.textures.top,
            1 => self.textures.bottom,
            _ => self.textures.side,
        };

        Some(Intersect {
            point,
            normal,
            distance,
            u,
            v,
            texture_id,
            ambient_occlusion: self.occlusion_at(&point, axis, sign),
            material: self.material,
        })
    }
}
