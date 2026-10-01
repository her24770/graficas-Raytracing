use std::collections::HashMap;

use nalgebra_glm::Vec3;

use crate::color::Color;
use crate::cube::{face_index, face_tangent_axes, Cube};
use crate::light::Light;
use crate::ray_intersect::{FaceTextures, Material};
use crate::scene::{Ambient, Scene};
use crate::texture::Texture;

const TEXTURE_FILES: [&str; 41] = [
    "grass_block_top",
    "grass_block_side",
    "dirt",
    "stone",
    "deepslate",
    "stone_bricks",
    "oak_log",
    "oak_planks",
    "spruce_planks",
    "oak_leaves",
    "water_still",
    "dirt_path_top",
    "farmland",
    "wheat_stage7",
    "glass",
    "obsidian",
    "nether_portal",
    "glowstone",
    "stripped_oak_log",
    // Piel marina/exotica.
    "sea_lantern",
    "prismarine",
    "prismarine_bricks",
    "dark_prismarine",
    "crying_obsidian",
    "amethyst_block",
    "sculk",
    // Piel nevada.
    "snow",
    "ice",
    // Piel desertica.
    "sand",
    "sandstone",
    // Piel mesa.
    "red_sand",
    "orange_terracotta",
    "yellow_terracotta",
    "brown_terracotta",
    // Detalles de vida y accesorios.
    "barrel_top",
    "barrel_side",
    "hay_block_top",
    "hay_block_side",
    "poppy",
    "dandelion",
    "sky_overworld",
];

const SKY_TEXTURE: usize = TEXTURE_FILES.len() - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Overworld,
    Marine,
    Snow,
    Desert,
    Mesa,
}

impl Biome {
    pub const ALL: [Biome; 5] = [
        Biome::Overworld,
        Biome::Marine,
        Biome::Snow,
        Biome::Desert,
        Biome::Mesa,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Overworld => "Overworld",
            Biome::Marine => "Marino",
            Biome::Snow => "Nevado",
            Biome::Desert => "Desertico",
            Biome::Mesa => "Mesa",
        }
    }

    // Color de acento, cuanto se mezcla en cielo/ambiente (0-1) y multiplicador
    // de intensidad del sol. Es la "atmosfera" propia de cada bioma.
    pub fn mood(self) -> (Color, f32, f32) {
        match self {
            Biome::Overworld => (Color::from_hex(0x000000), 0.0, 1.0),
            Biome::Marine => (Color::from_hex(0x1E5A66), 0.35, 0.85),
            Biome::Snow => (Color::from_hex(0x8FA8B8), 0.45, 0.7),
            Biome::Desert => (Color::from_hex(0xFFE9A8), 0.3, 1.25),
            Biome::Mesa => (Color::from_hex(0xC96A3D), 0.35, 1.05),
        }
    }
}

struct TerrainTextures {
    top: usize,
    side: usize,
    dirt: usize,
    stone: usize,
    deepslate: usize,
    water: usize,
}

// Solo el terreno (pasto/tierra/piedra/roca oscura/agua) cambia de piel; la
// cabana, la granja, el puente y el portal se mantienen iguales en todos los biomas.
fn terrain_textures(biome: Biome) -> TerrainTextures {
    match biome {
        Biome::Overworld => TerrainTextures {
            top: tex("grass_block_top"),
            side: tex("grass_block_side"),
            dirt: tex("dirt"),
            stone: tex("stone"),
            deepslate: tex("deepslate"),
            water: tex("water_still"),
        },
        Biome::Marine => TerrainTextures {
            top: tex("sea_lantern"),
            side: tex("prismarine"),
            dirt: tex("dark_prismarine"),
            stone: tex("prismarine_bricks"),
            deepslate: tex("crying_obsidian"),
            water: tex("water_still"),
        },
        Biome::Snow => TerrainTextures {
            top: tex("snow"),
            side: tex("snow"),
            dirt: tex("dirt"),
            stone: tex("stone"),
            deepslate: tex("deepslate"),
            water: tex("ice"),
        },
        Biome::Desert => TerrainTextures {
            top: tex("sand"),
            side: tex("sand"),
            dirt: tex("sandstone"),
            stone: tex("sandstone"),
            deepslate: tex("deepslate"),
            water: tex("water_still"),
        },
        Biome::Mesa => TerrainTextures {
            top: tex("red_sand"),
            side: tex("orange_terracotta"),
            dirt: tex("orange_terracotta"),
            stone: tex("yellow_terracotta"),
            deepslate: tex("brown_terracotta"),
            water: tex("water_still"),
        },
    }
}

const SIZE: i32 = 24;
const CENTER: f32 = 11.5;
// Franja extra de terreno agregada simetricamente alrededor de la isla original,
// sin mover ninguna estructura ya ubicada (cabana, granja, arboles, cueva).
const PAD: i32 = 5;
const RADIUS: f32 = 11.5 + PAD as f32;

const NEIGHBORS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

fn tex(name: &str) -> usize {
    TEXTURE_FILES
        .iter()
        .position(|file| *file == name)
        .unwrap_or_else(|| panic!("textura no registrada: {name}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Grass,
    Dirt,
    Stone,
    Deepslate,
    StoneBricks,
    StoneSlab,
    OakLog,
    OakPlanks,
    SprucePlanks,
    Leaves,
    Water,
    Droplet,
    DirtPath,
    Farmland,
    Wheat,
    Glass,
    Obsidian,
    Portal,
    Torch,
    Lantern,
    Fence,
    StrippedOakLog,
    Fern,
    Poppy,
    Dandelion,
    Rock,
    Barrel,
    HayBale,
    Cloud,
}

impl Block {
    fn is_opaque_full(self) -> bool {
        !matches!(
            self,
            Block::Water
                | Block::Droplet
                | Block::Glass
                | Block::Portal
                | Block::Torch
                | Block::Lantern
                | Block::Fence
                | Block::Wheat
                | Block::StoneSlab
                | Block::Fern
                | Block::Poppy
                | Block::Dandelion
                | Block::Rock
                | Block::Barrel
                | Block::HayBale
        )
    }

    fn appearance(self, biome: Biome) -> (FaceTextures, Material) {
        let terrain = terrain_textures(biome);
        const SOIL: Material = Material::matte(0.03, 4.0);
        const ROCK: Material = Material::matte(0.12, 14.0);
        const WOOD: Material = Material::matte(0.08, 10.0);
        const WATER: Material = Material {
            diffuse: 0.7,
            reflectivity: 0.3,
            transparency: 0.6,
            refractive_index: 1.33,
            animated: true,
            ..Material::matte(0.9, 150.0)
        };
        const GLASS: Material = Material {
            diffuse: 0.3,
            reflectivity: 0.05,
            transparency: 0.9,
            refractive_index: 1.5,
            ..Material::matte(0.8, 200.0)
        };
        const OBSIDIAN: Material = Material {
            reflectivity: 0.1,
            ..Material::matte(0.6, 80.0)
        };
        const PORTAL: Material = Material {
            reflectivity: 0.1,
            transparency: 0.3,
            emission: 0.9,
            ..Material::matte(0.2, 30.0)
        };
        const GLOW: Material = Material {
            emission: 1.0,
            ..Material::matte(0.0, 1.0)
        };

        let uniform = |name: &str| FaceTextures::uniform(tex(name));

        match self {
            Block::Grass => (
                FaceTextures::top_side_bottom(terrain.top, terrain.side, terrain.dirt),
                Material::matte(0.05, 8.0),
            ),
            Block::Dirt => (FaceTextures::uniform(terrain.dirt), SOIL),
            Block::Stone => (FaceTextures::uniform(terrain.stone), ROCK),
            Block::Deepslate => (FaceTextures::uniform(terrain.deepslate), ROCK),
            Block::StoneBricks | Block::StoneSlab => (uniform("stone_bricks"), ROCK),
            Block::OakLog => (uniform("oak_log"), WOOD),
            Block::StrippedOakLog => (uniform("stripped_oak_log"), WOOD),
            Block::OakPlanks | Block::Fence => (uniform("oak_planks"), WOOD),
            Block::SprucePlanks => (uniform("spruce_planks"), WOOD),
            Block::Leaves => (uniform("oak_leaves"), Material::matte(0.1, 18.0)),
            Block::Water | Block::Droplet => (FaceTextures::uniform(terrain.water), WATER),
            Block::DirtPath => (
                FaceTextures::top_side_bottom(tex("dirt_path_top"), tex("dirt"), tex("dirt")),
                SOIL,
            ),
            Block::Farmland => (
                FaceTextures::top_side_bottom(tex("farmland"), tex("dirt"), tex("dirt")),
                SOIL,
            ),
            Block::Wheat => (uniform("wheat_stage7"), Material::matte(0.05, 6.0)),
            Block::Glass => (uniform("glass"), GLASS),
            Block::Obsidian => (uniform("obsidian"), OBSIDIAN),
            Block::Portal => (uniform("nether_portal"), PORTAL),
            Block::Torch | Block::Lantern => (uniform("glowstone"), GLOW),
            Block::Fern => (uniform("oak_leaves"), Material::matte(0.05, 6.0)),
            Block::Poppy => (uniform("poppy"), Material::matte(0.05, 6.0)),
            Block::Dandelion => (uniform("dandelion"), Material::matte(0.05, 6.0)),
            // Las rocas sueltas usan la piedra propia de cada bioma: piedra normal
            // en el overworld, arenisca en el desierto, terracota en la mesa, etc.
            Block::Rock => (FaceTextures::uniform(terrain.stone), ROCK),
            Block::Barrel => (
                FaceTextures::top_side_bottom(tex("barrel_top"), tex("barrel_side"), tex("barrel_top")),
                WOOD,
            ),
            Block::HayBale => (
                FaceTextures::top_side_bottom(tex("hay_block_top"), tex("hay_block_side"), tex("hay_block_top")),
                Material::matte(0.05, 6.0),
            ),
            Block::Cloud => (uniform("snow"), Material::matte(0.0, 1.0)),
        }
    }
}

struct World {
    blocks: HashMap<(i32, i32, i32), Block>,
    lights: Vec<Light>,
}

impl World {
    fn new() -> Self {
        World {
            blocks: HashMap::new(),
            lights: Vec::new(),
        }
    }

    // Recibe coordenadas de bloque; las luces quedan en coordenadas de escena.
    fn add_light(&mut self, block_position: Vec3, color: u32, intensity: f32, range: f32) {
        let position = block_position - Vec3::new(CENTER, 0.0, CENTER);
        self.lights
            .push(Light::new(position, Color::from_hex(color), intensity, range));
    }

    fn get(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        self.blocks.get(&(x, y, z)).copied()
    }

    fn set(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.blocks.insert((x, y, z), block);
    }

    fn set_if_empty(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.blocks.entry((x, y, z)).or_insert(block);
    }

    fn remove(&mut self, x: i32, y: i32, z: i32) {
        self.blocks.remove(&(x, y, z));
    }

    fn is_opaque(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z).is_some_and(Block::is_opaque_full)
    }

    fn top_y(&self, x: i32, z: i32) -> Option<i32> {
        (-20..=20).rev().find(|&y| self.get(x, y, z).is_some())
    }
}

fn hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (z as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

// 0 en el centro de la isla, 1 en el borde; contorno redondeado-cuadrado con ondulaciones.
fn edge_distance(x: i32, z: i32) -> f32 {
    let dx = (x as f32 - CENTER) / RADIUS;
    let dz = (z as f32 - CENTER) / RADIUS;
    let theta = dz.atan2(dx);
    let wobble = 1.0 + 0.05 * (3.0 * theta + 0.5).sin() + 0.035 * (5.0 * theta + 2.0).sin();
    (dx.abs().powf(3.0) + dz.abs().powf(3.0)).powf(1.0 / 3.0) / wobble
}

fn surface_height(x: i32, z: i32) -> i32 {
    let jitter = if (6..=9).contains(&z) {
        0
    } else {
        (hash(x, z) % 2) as i32
    };

    if x + jitter <= 7 && (1..=12).contains(&z) {
        CLIFF_TOP
    } else if x + jitter <= 6 && (13..=16).contains(&z) {
        3
    } else if x <= 4 && z >= 17 {
        1
    } else {
        0
    }
}

const CLIFF_TOP: i32 = 6;

fn bottom_height(x: i32, z: i32, q: f32) -> i32 {
    let depth = 5.0 + 6.0 * (1.0 - q).max(0.0).powf(0.7);
    -(depth.round() as i32) - (hash(z, x) % 2) as i32
}

const RIVER_Z: std::ops::RangeInclusive<i32> = 15..=17;

fn is_river(x: i32, z: i32) -> bool {
    let pool = (9..=12).contains(&x) && (5..=11).contains(&z);
    let back_branch = (10..=12).contains(&x) && (1..=4).contains(&z);
    let fall_base = x == 8 && (7..=8).contains(&z);
    let bend = (11..=13).contains(&x) && (12..=14).contains(&z);
    let east = x >= 11 && RIVER_Z.contains(&z);
    pool || back_branch || fall_base || bend || east
}

fn build_terrain(world: &mut World) {
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            let q = edge_distance(x, z);
            if q > 1.0 {
                continue;
            }

            let top = surface_height(x, z);
            let bottom = bottom_height(x, z, q);
            let dirt_depth = 2 + (hash(x + 7, z) % 2) as i32;
            let deepslate_line = -6 + (hash(x, z + 3) % 2) as i32;

            for y in bottom..=top {
                let block = if y == top {
                    Block::Grass
                } else if y >= top - dirt_depth {
                    Block::Dirt
                } else if y > deepslate_line {
                    Block::Stone
                } else {
                    Block::Deepslate
                };
                world.set(x, y, z, block);
            }
        }
    }
}

fn build_water(world: &mut World) {
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            if is_river(x, z) && world.get(x, 0, z).is_some() && surface_height(x, z) == 0 {
                world.set(x, 0, z, Block::Water);
                world.set(x, -1, z, Block::Dirt);
            }
        }
    }

    // Arroyo sobre el acantilado y cascada principal.
    for x in 3..=7 {
        for z in 7..=8 {
            world.set(x, CLIFF_TOP, z, Block::Water);
        }
    }
    for y in 1..CLIFF_TOP {
        for z in 7..=8 {
            world.set(8, y, z, Block::Water);
        }
    }

    // Escalinata de piedra junto a la cascada.
    for (z, height) in [(9, 5), (10, 4), (11, 3), (12, 2), (13, 1)] {
        for y in 1..=height {
            world.set(8, y, z, Block::StoneBricks);
        }
    }

    // El rio cae por el borde este y se deshace en gotas.
    for z in RIVER_Z {
        let edge_x = (-PAD..SIZE + PAD).rev().find(|&x| world.get(x, 0, z).is_some()).unwrap();
        for y in -6..=0 {
            world.set(edge_x + 1, y, z, Block::Water);
        }
        for step in 0..4 {
            let y = -8 - step * 2 - (hash(z, step) % 2) as i32;
            let dx = (hash(step, z) % 2) as i32;
            world.set(edge_x + 1 + dx, y, z, Block::Droplet);
        }
    }
}

fn build_cabin(world: &mut World) {
    let (x0, x1, z0, z1) = (14, 19, 3, 8);

    for x in x0 + 1..x1 {
        for z in z0 + 1..z1 {
            world.set(x, 0, z, Block::OakPlanks);
        }
    }

    for y in 1..=3 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let on_x_wall = x == x0 || x == x1;
                let on_z_wall = z == z0 || z == z1;
                if !(on_x_wall || on_z_wall) {
                    continue;
                }
                let block = if on_x_wall && on_z_wall {
                    Block::OakLog
                } else {
                    Block::OakPlanks
                };
                world.set(x, y, z, block);
            }
        }
    }

    // Puerta hacia el camino y ventanas.
    world.remove(15, 1, z1);
    world.remove(15, 2, z1);
    world.set(17, 2, z1, Block::Glass);
    world.set(x1, 2, 5, Block::Glass);
    world.set(x1, 2, 6, Block::Glass);

    // Techo escalonado a dos aguas con alero.
    for (y, from, to) in [(4, z0 - 1, z1 + 1), (5, z0, z1), (6, z0 + 1, z1 - 1), (7, z0 + 2, z1 - 2)] {
        for x in x0 - 1..=x1 + 1 {
            for z in from..=to {
                world.set(x, y, z, Block::SprucePlanks);
            }
        }
    }

    for y in 1..=9 {
        world.set(x0 + 1, y, z0 + 1, Block::StoneBricks);
    }
}

fn build_farm(world: &mut World) {
    for x in 16..=21 {
        for z in 9..=14 {
            let border = x == 16 || x == 21 || z == 9 || z == 14;
            if border {
                world.set(x, 1, z, Block::Fence);
            } else {
                world.set(x, 0, z, Block::Farmland);
                world.set(x, 1, z, Block::Wheat);
            }
        }
    }
}

fn build_paths(world: &mut World) {
    for x in 14..=15 {
        for z in 9..=13 {
            world.set(x, 0, z, Block::DirtPath);
        }
    }
    for x in 6..=9 {
        for z in 18..=20 {
            if world.get(x, 0, z).is_some() {
                world.set(x, 0, z, Block::DirtPath);
            }
        }
    }
}

fn build_bridge(world: &mut World) {
    let (first, last) = (*RIVER_Z.start(), *RIVER_Z.end());
    // El tablero mide 4 de ancho (13..16); los dos del medio se caminan,
    // los de afuera sostienen la baranda de vallas.
    for x in 13..=16 {
        world.set(x, 1, first - 1, Block::StoneSlab);
        for z in RIVER_Z {
            world.set(x, 1, z, Block::StoneBricks);
        }
        world.set(x, 1, last + 1, Block::StoneSlab);
    }
    for x in [13, 16] {
        for z in RIVER_Z {
            world.set(x, 2, z, Block::Fence);
        }
    }
}

fn build_dock(world: &mut World) {
    for x in 11..=13 {
        world.set(x, 0, 8, Block::OakPlanks);
    }
    world.set(11, -1, 8, Block::StrippedOakLog);
    world.set(11, -2, 8, Block::StrippedOakLog);
}

// Pequeno pozo/mirador cubierto junto al camino, estructura nueva (no solo relleno).
fn build_well(world: &mut World) {
    let (x0, z0) = (12, 13);
    for dx in 0..=1 {
        for dz in 0..=1 {
            world.set(x0 + dx, 0, z0 + dz, Block::StoneBricks);
        }
    }
    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        for y in 1..=3 {
            world.set(x0 + dx, y, z0 + dz, Block::OakLog);
        }
    }
    for dx in 0..=1 {
        for dz in 0..=1 {
            world.set(x0 + dx, 4, z0 + dz, Block::SprucePlanks);
        }
    }
}

// Torre mirador con baranda (vallas) arriba, sobre el pasto nuevo del borde.
fn build_watchtower(world: &mut World, x: i32, z: i32) {
    let Some(ground) = world.top_y(x, z) else {
        return;
    };

    for y in ground + 1..=ground + 6 {
        world.set(x, y, z, Block::StoneBricks);
    }

    let platform_y = ground + 7;
    for dx in -1..=1 {
        for dz in -1..=1 {
            world.set(x + dx, platform_y, z + dz, Block::StoneBricks);
        }
    }

    let rail_y = platform_y + 1;
    for dx in -1i32..=1 {
        for dz in -1i32..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            if dx.abs() == 1 && dz.abs() == 1 {
                continue;
            }
            world.set(x + dx, rail_y, z + dz, Block::Fence);
        }
    }

    world.set(x, rail_y, z, Block::Lantern);
    world.add_light(
        Vec3::new(x as f32, rail_y as f32 - 0.2, z as f32),
        0xFFC080,
        1.6,
        9.0,
    );
}

// Rocas sueltas bajo la isla: refuerza la idea de isla "deconstruida" que se cae a pedazos.
fn build_floating_debris(world: &mut World) {
    let clusters: [(i32, i32, i32); 4] = [(5, -14, 10), (19, -13, 4), (1, -15, 19), (23, -12, 16)];

    for (cx, cy, cz) in clusters {
        for (dx, dy, dz) in [(0i32, 0i32, 0i32), (1, 0, 0), (0, 0, 1), (0, -1, 0)] {
            let block = if (dx + dy + dz).abs() % 2 == 0 {
                Block::Stone
            } else {
                Block::Deepslate
            };
            world.set(cx + dx, cy + dy, cz + dz, block);
        }
    }
}

// Dispersa fernas, flores y rocas sueltas por todo el pasto libre, para que la
// isla no se vea plana. Se corre al final, asi solo ocupa celdas que quedaron vacias.
fn build_ground_details(world: &mut World) {
    let mut sites = Vec::new();
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            sites.push((x, z));
        }
    }

    for (x, z) in sites {
        let Some(top) = world.top_y(x, z) else {
            continue;
        };
        if world.get(x, top, z) != Some(Block::Grass) || world.get(x, top + 1, z).is_some() {
            continue;
        }

        let block = match hash(x * 17 + 3, z * 31 + 11) % 20 {
            0 | 1 | 2 => Some(Block::Fern),
            3 => Some(Block::Poppy),
            4 => Some(Block::Dandelion),
            5 => Some(Block::Rock),
            _ => None,
        };

        if let Some(block) = block {
            world.set(x, top + 1, z, block);
        }
    }
}

// Barriles y paja junto a la cabana, como en un caserio con vida.
fn build_cabin_details(world: &mut World) {
    world.set(13, 1, 6, Block::Barrel);
    world.set(13, 1, 7, Block::Barrel);
    world.set(13, 1, 5, Block::HayBale);
}

fn build_cloud(world: &mut World, cx: i32, cy: i32, cz: i32) {
    for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        if hash(cx + dx * 3, cz + dz * 5) % 4 != 0 {
            world.set(cx + dx, cy, cz + dz, Block::Cloud);
        }
    }
}

// Nubes de bloques flotando bien arriba de la isla, visibles desde cualquier
// angulo de la orbita.
fn build_clouds(world: &mut World) {
    let positions: [(i32, i32, i32); 6] = [
        (-10, 20, 5),
        (10, 23, -9),
        (36, 19, 11),
        (5, 25, 31),
        (29, 21, 27),
        (-9, 24, 23),
    ];
    for (x, y, z) in positions {
        build_cloud(world, x, y, z);
    }
}

fn build_tree(
    world: &mut World,
    x: i32,
    z: i32,
    base_y: i32,
    trunk: i32,
    radius: f32,
    trunk_block: Block,
) {
    for y in base_y..base_y + trunk {
        world.set(x, y, z, trunk_block);
    }

    let top = base_y + trunk;
    let r = radius.ceil() as i32;
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                let dist = ((dx * dx) as f32 + (dy * dy) as f32 * 1.6 + (dz * dz) as f32).sqrt();
                let ragged = (hash(x + dx * 3 + dy, z + dz * 5) % 3) as f32 * 0.35;
                if dist + ragged <= radius + 0.3 {
                    world.set_if_empty(x + dx, top + dy, z + dz, Block::Leaves);
                }
            }
        }
    }
}

fn build_vegetation(world: &mut World) {
    let above_cliff = CLIFF_TOP + 1;
    build_tree(world, 2, 10, above_cliff, 5, 3.0, Block::OakLog);
    build_tree(world, 3, 4, above_cliff, 5, 2.8, Block::OakLog);
    build_tree(world, 13, 3, 1, 8, 3.4, Block::OakLog);

    // Arboles y arbustos sueltos en la franja de pasto nueva, para que no quede vacia.
    build_tree(world, -2, 13, 1, 6, 2.6, Block::OakLog);
    build_tree(world, -3, 20, 1, 5, 2.4, Block::OakLog);
    build_tree(world, 25, 7, 1, 6, 2.8, Block::OakLog);
    build_tree(world, 24, 18, 1, 5, 2.4, Block::OakLog);
    // Tronco claro (tipo abedul) para variar el paisaje.
    build_tree(world, 22, 5, 1, 6, 2.6, Block::StrippedOakLog);

    for (x, z, height) in [
        (5, 14, 2),
        (6, 14, 1),
        (6, 15, 1),
        (5, 15, 1),
        (4, 16, 1),
        (0, 8, 1),
        (-2, 4, 1),
        (1, 20, 1),
        (23, 10, 1),
        (22, 21, 1),
    ] {
        if let Some(top) = world.top_y(x, z) {
            for y in top + 1..=top + height {
                world.set_if_empty(x, y, z, Block::Leaves);
            }
        }
    }

    for z in 2..=5 {
        world.set(7, above_cliff, z, Block::Fence);
    }
    for x in 5..=6 {
        world.set(x, above_cliff, 2, Block::Fence);
    }
}

fn build_portal_cave(world: &mut World) {
    let cave_x = 8..=16;
    let walls: Vec<i32> = cave_x
        .clone()
        .map(|x| (-PAD..SIZE + PAD).rev().find(|&z| world.get(x, -3, z).is_some()).unwrap())
        .collect();
    let back = walls.iter().min().unwrap() - 2;

    for (x, wall) in cave_x.zip(&walls) {
        for y in -5..=-2 {
            for z in back + 1..=*wall {
                world.remove(x, y, z);
            }
        }
    }

    let frame_z = back + 1;
    for x in 11..=14 {
        world.set(x, -6, frame_z, Block::Obsidian);
        world.set(x, -1, frame_z, Block::Obsidian);
    }
    for y in -5..=-2 {
        world.set(11, y, frame_z, Block::Obsidian);
        world.set(14, y, frame_z, Block::Obsidian);
        world.set(12, y, frame_z, Block::Portal);
        world.set(13, y, frame_z, Block::Portal);
    }
    for x in 8..=16 {
        world.set_if_empty(x, -6, frame_z, Block::Stone);
    }

    world.set(9, -3, frame_z, Block::Torch);

    let front = frame_z as f32 + 0.6;
    world.add_light(Vec3::new(9.0, -2.9, front), 0xFFAA50, 1.6, 6.0);
    world.add_light(Vec3::new(12.5, -3.5, front), 0xAA46FF, 1.8, 7.0);
}

fn build_lanterns(world: &mut World) {
    for (x, z) in [(13, 9), (16, 18)] {
        let Some(ground) = world.top_y(x, z) else {
            continue;
        };
        world.set(x, ground + 1, z, Block::Fence);
        world.set(x, ground + 2, z, Block::Fence);
        world.set(x, ground + 3, z, Block::Lantern);
        world.add_light(
            Vec3::new(x as f32, ground as f32 + 2.8, z as f32),
            0xFFBE6E,
            1.4,
            7.0,
        );
    }

    // Lampara dentro de la cabana: su luz sale por la puerta.
    world.set(16, 3, 5, Block::Lantern);
    world.add_light(Vec3::new(16.0, 2.8, 5.0), 0xFFB060, 1.5, 6.0);
}

fn block_shapes(world: &World, block: Block, x: i32, y: i32, z: i32) -> Vec<(Vec3, Vec3)> {
    let full = (Vec3::zeros(), Vec3::new(1.0, 1.0, 1.0));

    match block {
        Block::Water => {
            let above = world.get(x, y + 1, z) == Some(Block::Water);
            let below = world.get(x, y - 1, z) == Some(Block::Water);
            if above || below {
                vec![full]
            } else {
                vec![(Vec3::new(0.0, -0.1, 0.0), Vec3::new(1.0, 0.8, 1.0))]
            }
        }
        Block::Droplet => {
            let jitter = (hash(x, y) % 5) as f32 * 0.1 - 0.2;
            vec![(Vec3::new(jitter, 0.0, -jitter), Vec3::new(0.3, 0.3, 0.3))]
        }
        Block::StoneSlab => vec![(Vec3::new(0.0, -0.25, 0.0), Vec3::new(1.0, 0.5, 1.0))],
        Block::Torch => vec![(Vec3::new(0.0, -0.2, 0.0), Vec3::new(0.16, 0.6, 0.16))],
        Block::Lantern => vec![(Vec3::new(0.0, -0.25, 0.0), Vec3::new(0.4, 0.45, 0.4))],
        Block::Portal => vec![(Vec3::zeros(), Vec3::new(1.0, 1.0, 0.25))],
        Block::Wheat => vec![(Vec3::new(0.0, -0.1, 0.0), Vec3::new(0.9, 0.8, 0.9))],
        Block::Fence => {
            let mut parts = vec![(Vec3::zeros(), Vec3::new(0.25, 1.0, 0.25))];
            for (dx, dz) in [(1, 0), (0, 1)] {
                if world.get(x + dx, y, z + dz) == Some(Block::Fence) {
                    let size = Vec3::new(
                        if dx == 1 { 1.0 } else { 0.12 },
                        0.12,
                        if dz == 1 { 1.0 } else { 0.12 },
                    );
                    for rail_y in [0.25, -0.1] {
                        let offset = Vec3::new(dx as f32 * 0.5, rail_y, dz as f32 * 0.5);
                        parts.push((offset, size));
                    }
                }
            }
            parts
        }
        Block::Fern | Block::Poppy | Block::Dandelion => {
            let jitter = (hash(x + 5, z + 9) % 5) as f32 * 0.04 - 0.08;
            vec![(
                Vec3::new(jitter, -0.3, -jitter),
                Vec3::new(0.55, 0.4, 0.55),
            )]
        }
        Block::Rock => {
            let size = 0.45 + (hash(x, z) % 4) as f32 * 0.08;
            let jitter = (hash(x + 3, z + 7) % 5) as f32 * 0.05 - 0.1;
            vec![(
                Vec3::new(jitter, -0.5 + size * 0.5, -jitter),
                Vec3::new(size, size, size),
            )]
        }
        Block::Barrel => vec![(Vec3::new(0.0, -0.15, 0.0), Vec3::new(0.75, 0.7, 0.75))],
        Block::Cloud => vec![(Vec3::zeros(), Vec3::new(1.0, 0.55, 1.0))],
        _ => vec![full],
    }
}

const OCCLUSION_LEVELS: [f32; 4] = [1.0, 0.78, 0.6, 0.45];

// Oclusion por esquina al estilo de la iluminacion suave de Minecraft: una esquina
// se oscurece segun cuantos de sus tres vecinos frente a la cara estan ocupados.
fn face_occlusion(world: &World, position: [i32; 3], axis: usize, positive: bool) -> [f32; 4] {
    let mut front = position;
    front[axis] += if positive { 1 } else { -1 };

    let (b, c) = face_tangent_axes(axis);
    let solid = |offset_b: i32, offset_c: i32| {
        let mut cell = front;
        cell[b] += offset_b;
        cell[c] += offset_c;
        world.is_opaque(cell[0], cell[1], cell[2])
    };

    let mut corners = [1.0; 4];
    for j in 0..2 {
        for i in 0..2 {
            let db = if i == 0 { -1 } else { 1 };
            let dc = if j == 0 { -1 } else { 1 };
            let side_b = solid(db, 0);
            let side_c = solid(0, dc);
            let level = if side_b && side_c {
                3
            } else {
                side_b as usize + side_c as usize + solid(db, dc) as usize
            };
            corners[i + 2 * j] = OCCLUSION_LEVELS[level];
        }
    }
    corners
}

fn to_cubes(world: &World, biome: Biome) -> Vec<Cube> {
    let mut cubes = Vec::new();

    for (&(x, y, z), &block) in &world.blocks {
        let exposed = NEIGHBORS
            .iter()
            .any(|&(dx, dy, dz)| !world.is_opaque(x + dx, y + dy, z + dz));
        if !exposed {
            continue;
        }

        let (textures, material) = block.appearance(biome);
        let center = Vec3::new(x as f32 - CENTER, y as f32, z as f32 - CENTER);

        for (offset, size) in block_shapes(world, block, x, y, z) {
            let mut cube = Cube::new(center + offset, size, material, textures);

            if block.is_opaque_full() {
                for axis in 0..3 {
                    for positive in [false, true] {
                        cube.ambient_occlusion[face_index(axis, positive)] =
                            face_occlusion(world, [x, y, z], axis, positive);
                    }
                }
            }

            cubes.push(cube);
        }
    }

    cubes
}

pub fn build_diorama(biome: Biome) -> Scene {
    let textures = TEXTURE_FILES
        .iter()
        .map(|name| Texture::from_bmp(&format!("assets/textures/{name}.bmp")))
        .collect();

    let mut world = World::new();
    build_terrain(&mut world);
    build_water(&mut world);
    build_cabin(&mut world);
    build_farm(&mut world);
    build_paths(&mut world);
    build_bridge(&mut world);
    build_dock(&mut world);
    build_well(&mut world);
    build_vegetation(&mut world);
    build_portal_cave(&mut world);
    build_lanterns(&mut world);
    build_watchtower(&mut world, 24, 10);
    build_floating_debris(&mut world);
    build_cabin_details(&mut world);
    build_clouds(&mut world);
    build_ground_details(&mut world);

    // Arranca en la hora dorada del atardecer; el ciclo completo vive en daycycle.rs.
    let (sun, ambient, sky) = crate::daycycle::lighting_at(0.78);
    let mut lights = vec![sun];
    lights.append(&mut world.lights);

    Scene::new(to_cubes(&world, biome), textures, lights, ambient, sky, SKY_TEXTURE)
}
