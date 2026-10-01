use crate::color::Color;
use crate::cube::Cube;
use crate::light::Light;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::texture::Texture;
use nalgebra_glm::Vec3;

const EPSILON: f32 = 1e-4;
const CELL_SIZE: f32 = 1.0;

// Luz ambiente hemisferica: las caras que miran arriba reciben el tono del cielo
// y las que miran abajo el rebote calido del suelo.
pub struct Ambient {
    pub sky: Color,
    pub ground: Color,
    pub intensity: f32,
}

// Degradado del fondo (no geometria) segun la altura del rayo: horizonte, medio y cenit.
pub struct SkyGradient {
    pub horizon: Color,
    pub middle: Color,
    pub high: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realm {
    Overworld,
    Nether,
    End,
}

// Zona que teletransporta a la camara libre cuando el jugador la toca.
pub struct Portal {
    pub min: Vec3,
    pub max: Vec3,
    pub destination: Realm,
}

// Donde aparece el jugador en esta escena cuando llega desde `from`.
pub struct Arrival {
    pub from: Realm,
    pub eye: Vec3,
    pub yaw: f32,
}

pub struct Scene {
    pub cubes: Vec<Cube>,
    pub textures: Vec<Texture>,
    pub lights: Vec<Light>,
    pub ambient: Ambient,
    pub sky: SkyGradient,
    pub sky_texture: usize,
    pub portals: Vec<Portal>,
    pub arrivals: Vec<Arrival>,
    bounds_min: Vec3,
    bounds_max: Vec3,
    dims: [usize; 3],
    cells: Vec<Vec<u32>>,
}

impl Scene {
    pub fn new(
        cubes: Vec<Cube>,
        textures: Vec<Texture>,
        lights: Vec<Light>,
        ambient: Ambient,
        sky: SkyGradient,
        sky_texture: usize,
    ) -> Self {
        let mut bounds_min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut bounds_max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for cube in &cubes {
            for axis in 0..3 {
                bounds_min[axis] = bounds_min[axis].min(cube.min[axis]);
                bounds_max[axis] = bounds_max[axis].max(cube.max[axis]);
            }
        }

        let dims = [0, 1, 2].map(|axis| {
            (((bounds_max[axis] - bounds_min[axis]) / CELL_SIZE).ceil() as usize).max(1)
        });

        let mut cells = vec![Vec::new(); dims[0] * dims[1] * dims[2]];

        for (index, cube) in cubes.iter().enumerate() {
            let low = [0, 1, 2].map(|axis| cell_coord(cube.min[axis], bounds_min[axis], dims[axis]));
            let high = [0, 1, 2]
                .map(|axis| cell_coord(cube.max[axis] - EPSILON, bounds_min[axis], dims[axis]));

            for z in low[2]..=high[2] {
                for y in low[1]..=high[1] {
                    for x in low[0]..=high[0] {
                        cells[(z * dims[1] + y) * dims[0] + x].push(index as u32);
                    }
                }
            }
        }

        Scene {
            cubes,
            textures,
            lights,
            ambient,
            sky,
            sky_texture,
            portals: Vec::new(),
            arrivals: Vec::new(),
            bounds_min,
            bounds_max,
            dims,
            cells,
        }
    }

    fn bounds_range(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<(f32, f32)> {
        let mut t_near = f32::NEG_INFINITY;
        let mut t_far = f32::INFINITY;

        for axis in 0..3 {
            let origin = ray_origin[axis];
            let dir = ray_direction[axis];

            if dir.abs() < EPSILON {
                if origin < self.bounds_min[axis] || origin > self.bounds_max[axis] {
                    return None;
                }
                continue;
            }

            let t1 = (self.bounds_min[axis] - origin) / dir;
            let t2 = (self.bounds_max[axis] - origin) / dir;
            t_near = t_near.max(t1.min(t2));
            t_far = t_far.min(t1.max(t2));

            if t_near > t_far {
                return None;
            }
        }

        (t_far > EPSILON).then_some((t_near, t_far))
    }

    pub fn portal_touching(&self, min: &Vec3, max: &Vec3) -> Option<Realm> {
        self.portals
            .iter()
            .find(|portal| (0..3).all(|axis| min[axis] < portal.max[axis] && max[axis] > portal.min[axis]))
            .map(|portal| portal.destination)
    }

    pub fn arrival_from(&self, from: Realm) -> Option<&Arrival> {
        self.arrivals.iter().find(|arrival| arrival.from == from)
    }

    // Colision para la camara libre: true si la caja toca algun cubo solido.
    // Usa la misma grilla que los rayos, asi solo mira los cubos cercanos.
    pub fn blocks_box(&self, min: &Vec3, max: &Vec3) -> bool {
        for axis in 0..3 {
            if max[axis] <= self.bounds_min[axis] || min[axis] >= self.bounds_max[axis] {
                return false;
            }
        }

        let low = [0, 1, 2].map(|axis| cell_coord(min[axis], self.bounds_min[axis], self.dims[axis]));
        let high = [0, 1, 2].map(|axis| cell_coord(max[axis], self.bounds_min[axis], self.dims[axis]));

        for z in low[2]..=high[2] {
            for y in low[1]..=high[1] {
                for x in low[0]..=high[0] {
                    let touches = self.cells[(z * self.dims[1] + y) * self.dims[0] + x]
                        .iter()
                        .map(|&index| &self.cubes[index as usize])
                        .any(|cube| {
                            cube.solid
                                && (0..3).all(|axis| min[axis] < cube.max[axis] && max[axis] > cube.min[axis])
                        });
                    if touches {
                        return true;
                    }
                }
            }
        }

        false
    }

    // Recorrido de grilla de Amanatides-Woo: visita solo las celdas que cruza el rayo.
    pub fn trace(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<Intersect> {
        let (t_enter, t_exit) = self.bounds_range(ray_origin, ray_direction)?;
        let entry = ray_origin + ray_direction * t_enter.max(0.0);

        let mut cell = [0, 1, 2].map(|axis| {
            cell_coord(entry[axis], self.bounds_min[axis], self.dims[axis]) as i32
        });
        let mut step = [0i32; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];

        for axis in 0..3 {
            let dir = ray_direction[axis];
            if dir.abs() < EPSILON {
                continue;
            }
            step[axis] = if dir > 0.0 { 1 } else { -1 };
            let next_cell = cell[axis] + if dir > 0.0 { 1 } else { 0 };
            let boundary = self.bounds_min[axis] + next_cell as f32 * CELL_SIZE;
            t_max[axis] = (boundary - ray_origin[axis]) / dir;
            t_delta[axis] = CELL_SIZE / dir.abs();
        }

        let mut closest: Option<Intersect> = None;

        loop {
            let index = (cell[2] as usize * self.dims[1] + cell[1] as usize) * self.dims[0]
                + cell[0] as usize;

            for &cube_index in &self.cells[index] {
                if let Some(hit) = self.cubes[cube_index as usize].ray_intersect(ray_origin, ray_direction) {
                    if closest.is_none_or(|current| hit.distance < current.distance) {
                        closest = Some(hit);
                    }
                }
            }

            let axis = if t_max[0] < t_max[1] {
                if t_max[0] < t_max[2] { 0 } else { 2 }
            } else if t_max[1] < t_max[2] {
                1
            } else {
                2
            };

            if closest.is_some_and(|hit| hit.distance <= t_max[axis]) || t_max[axis] > t_exit {
                return closest;
            }

            cell[axis] += step[axis];
            if cell[axis] < 0 || cell[axis] >= self.dims[axis] as i32 {
                return closest;
            }
            t_max[axis] += t_delta[axis];
        }
    }
}

fn cell_coord(value: f32, min: f32, dim: usize) -> usize {
    (((value - min) / CELL_SIZE).floor().max(0.0) as usize).min(dim - 1)
}
