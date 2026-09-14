use crate::color::Color;
use crate::cube::Cube;
use crate::texture::Texture;

pub struct Scene {
    pub cubes: Vec<Cube>,
    pub textures: Vec<Texture>,
    pub background_color: Color,
}

impl Scene {
    pub fn new(cubes: Vec<Cube>, textures: Vec<Texture>, background_color: Color) -> Self {
        Scene {
            cubes,
            textures,
            background_color,
        }
    }
}
