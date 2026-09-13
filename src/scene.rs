use crate::color::Color;
use crate::cube::Cube;

pub struct Scene {
    pub cubes: Vec<Cube>,
    pub background_color: Color,
}

impl Scene {
    pub fn new(cubes: Vec<Cube>, background_color: Color) -> Self {
        Scene {
            cubes,
            background_color,
        }
    }
}
