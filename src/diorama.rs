use std::collections::HashMap;

use nalgebra_glm::Vec3;

use crate::color::Color;
use crate::cube::{face_index, face_tangent_axes, Cube};
use crate::light::Light;
use crate::ray_intersect::{FaceTextures, Material};
use crate::scene::{Arrival, Portal, Realm, Scene};
use crate::texture::Texture;

const TEXTURE_FILES: [&str; 50] = [
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
    "campfire_log",
    "campfire_fire",
    "cherry_log",
    "cherry_log_top",
    "cherry_leaves",
    "cherry_planks",
    "end_portal_frame_top",
    "end_portal_frame_side",
    "end_portal",
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
    Sakura,
}

impl Biome {
    pub const ALL: [Biome; 6] = [
        Biome::Overworld,
        Biome::Marine,
        Biome::Snow,
        Biome::Desert,
        Biome::Mesa,
        Biome::Sakura,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Overworld => "Overworld",
            Biome::Marine => "Marino",
            Biome::Snow => "Nevado",
            Biome::Desert => "Desertico",
            Biome::Mesa => "Mesa",
            Biome::Sakura => "Sakura",
        }
    }

    // Color de acento, cuanto se mezcla en cielo/ambiente (0-1) y multiplicador
    // de intensidad del sol. Es la "atmosfera" propia de cada bioma.
    pub fn mood(self) -> (Color, f32, f32) {
        match self {
            Biome::Overworld => (Color::from_hex(0x000000), 0.0, 1.0),
            Biome::Marine => (Color::from_hex(0x1E5A66), 0.45, 0.9), // Más vibrante
            Biome::Snow => (Color::from_hex(0x8FA8B8), 0.6, 0.75), // Más blanco/frío
            Biome::Desert => (Color::from_hex(0xFFE9A8), 0.4, 1.3), // Más brillante
            Biome::Mesa => (Color::from_hex(0xC96A3D), 0.45, 1.1), // Más naranja
            Biome::Sakura => (Color::from_hex(0xFF88AA), 0.7, 1.2), // Mucho más rosado y cute
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
            dirt: tex("ice"),
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
        // El terreno del Sakura es el mismo pasto normal; lo que cambia son los arboles.
        Biome::Sakura => TerrainTextures {
            top: tex("grass_block_top"),
            side: tex("grass_block_side"),
            dirt: tex("dirt"),
            stone: tex("stone"),
            deepslate: tex("deepslate"),
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
    Campfire,
    SeaLantern,
    Snowflake,
    Petal,
    EndFrame,
    EndPortal,
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
                | Block::Campfire
                | Block::EndFrame
                | Block::EndPortal
        )
    }

    // Lo que la camara libre no puede atravesar. Agua, portal, plantas chicas,
    // piedritas y particulas se dejan pasar, igual que en Minecraft.
    fn blocks_movement(self) -> bool {
        !matches!(
            self,
            Block::Water
                | Block::Droplet
                | Block::Portal
                | Block::Torch
                | Block::Lantern
                | Block::Wheat
                | Block::Fern
                | Block::Poppy
                | Block::Dandelion
                | Block::Rock
                | Block::Cloud
                | Block::Snowflake
                | Block::Petal
                | Block::EndPortal
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
            emission: 1.2, // Reducido un poco para que no sature
            animated: true,
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
            Block::OakLog if biome == Biome::Sakura => (
                FaceTextures::top_side_bottom(tex("cherry_log_top"), tex("cherry_log"), tex("cherry_log_top")),
                WOOD,
            ),
            Block::StrippedOakLog if biome == Biome::Sakura => (uniform("cherry_log"), WOOD),
            Block::OakPlanks | Block::Fence | Block::SprucePlanks if biome == Biome::Sakura => (uniform("cherry_planks"), WOOD),
            
            Block::OakLog => (uniform("oak_log"), WOOD),
            Block::StrippedOakLog => (uniform("stripped_oak_log"), WOOD),
            Block::OakPlanks | Block::Fence => (uniform("oak_planks"), WOOD),
            Block::SprucePlanks => (uniform("spruce_planks"), WOOD),
            Block::Leaves if biome == Biome::Sakura => {
                (uniform("cherry_leaves"), Material::matte(0.1, 18.0))
            }
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
            Block::EndFrame => (
                FaceTextures::top_side_bottom(
                    tex("end_portal_frame_top"),
                    tex("end_portal_frame_side"),
                    tex("end_portal_frame_side"),
                ),
                ROCK,
            ),
            Block::EndPortal => (
                uniform("end_portal"),
                Material {
                    emission: 1.0,
                    animated: true,
                    ..Material::matte(0.0, 1.0)
                },
            ),
            Block::Torch => (uniform("glowstone"), GLOW),
            Block::Lantern => (uniform("glowstone"), GLOW),
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
            Block::Snowflake => (uniform("snow"), Material::matte(0.2, 5.0)),
            Block::Petal => (uniform("cherry_leaves"), Material::matte(0.2, 5.0)),
            Block::Campfire => (
                FaceTextures::top_side_bottom(tex("campfire_fire"), tex("campfire_log"), tex("campfire_log")),
                GLOW,
            ),
            Block::SeaLantern => (uniform("sea_lantern"), GLOW),
        }
    }
}

struct World {
    blocks: HashMap<(i32, i32, i32), Block>,
    lights: Vec<Light>,
    arrivals: Vec<Arrival>,
}

impl World {
    fn new() -> Self {
        World {
            blocks: HashMap::new(),
            lights: Vec::new(),
            arrivals: Vec::new(),
        }
    }

    // Punto donde aparece el jugador al llegar desde otro mundo, en coordenadas de bloque.
    fn add_arrival(&mut self, from: Realm, block_position: Vec3, yaw: f32) {
        let eye = block_position - Vec3::new(CENTER, 0.0, CENTER);
        self.arrivals.push(Arrival { from, eye, yaw });
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
    let base_depth = 5.0 + 8.0 * (1.0 - q).max(0.0).powf(0.7);
    // Menos frecuentes y mas cortas que antes: quedaba muy picudo y cargado por abajo.
    let stalactite = if hash(x * 3, z * 7) % 18 == 0 { (hash(x, z) % 3) as i32 } else { 0 };
    -(base_depth.round() as i32) - (hash(z, x) % 2) as i32 - stalactite
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
                    // Parches de ladrillo de piedra rajado entre la piedra, para que
                    // la pared del corte no sea un solo material parejo.
                    if hash(x * 3 + y * 13, z * 5 - y * 7) % 6 == 0 {
                        Block::StoneBricks
                    } else {
                        Block::Stone
                    }
                } else if hash(x * 7 + y * 17, z * 11 - y * 3) % 8 == 0 {
                    Block::Obsidian
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

    // Luces submarinas mágicas
    world.set(11, -1, 16, Block::SeaLantern);
    world.add_light(Vec3::new(11.0, -0.8, 16.0), 0x55FFFF, 2.0, 15.0);
    world.set(9, -1, 9, Block::SeaLantern);
    world.add_light(Vec3::new(9.0, -0.8, 9.0), 0x55FFFF, 2.0, 15.0);
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

    // Luz interior para que las ventanas brillen de noche.
    world.set(17, 2, 6, Block::Torch);
    world.add_light(Vec3::new(17.0, 2.0, 6.0), 0xFFB060, 2.5, 15.0);

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

fn column_bottom(world: &World, x: i32, z: i32, top: i32) -> i32 {
    let mut y = top;
    while world.get(x, y - 1, z).is_some() {
        y -= 1;
    }
    y
}

// Salientes y hendiduras en el borde de la isla: sin esto, la pared del corte
// es una franja lisa y pareja. Sobresaltos y huecos le dan relieve real.
fn build_cliff_details(world: &mut World) {
    let mut sites = Vec::new();
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            sites.push((x, z));
        }
    }

    for (x, z) in sites {
        let q = edge_distance(x, z);
        if !(0.72..=1.0).contains(&q) {
            continue;
        }
        let Some(top) = world.top_y(x, z) else {
            continue;
        };
        if world.get(x, top, z) != Some(Block::Grass) {
            continue;
        }
        let bottom = column_bottom(world, x, z, top);
        if top - bottom < 3 {
            continue;
        }

        let dx = (x as f32 - CENTER).signum() as i32;
        let dz = (z as f32 - CENTER).signum() as i32;
        if dx == 0 && dz == 0 {
            continue;
        }

        let roll = hash(x * 19 + 5, z * 23 + 7) % 6;
        let y = bottom + 1 + (hash(x + 2, z - 2) % (top - bottom - 2).max(1) as u32) as i32;

        match roll {
            // Saliente: un bloque extra hacia afuera, repitiendo el material de esa altura.
            0 | 1 => {
                if let Some(block) = world.get(x, y, z) {
                    if world.get(x + dx, y, z).is_none() {
                        world.set(x + dx, y, z, block);
                    }
                }
            }
            2 => {
                if let Some(block) = world.get(x, y, z) {
                    if world.get(x, y, z + dz).is_none() {
                        world.set(x, y, z + dz, block);
                    }
                }
            }
            // Hendidura: se saca un bloque interior, deja una sombra/hueco en la pared.
            3 | 4 => {
                if y > bottom + 1 && y < top {
                    world.remove(x, y, z);
                }
            }
            _ => {}
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

        // Bajamos la densidad a la mitad (antes 6 de 20, ahora 6 de 40): quedaba muy cargado.
        let block = match hash(x * 17 + 3, z * 31 + 11) % 40 {
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

fn build_camp(world: &mut World) {
    let (cx, cz) = (2, 16);
    let Some(ground) = world.top_y(cx, cz) else { return; };
    let y = ground + 1;
    
    world.set(cx, y, cz, Block::Campfire);
    world.add_light(Vec3::new(cx as f32, y as f32 + 0.2, cz as f32), 0xFF8833, 3.5, 20.0);
    
    // Troncos para sentarse alrededor
    if world.top_y(cx + 2, cz) == Some(ground) { world.set(cx + 2, y, cz, Block::OakLog); }
    if world.top_y(cx - 2, cz) == Some(ground) { world.set(cx - 2, y, cz, Block::OakLog); }
    if world.top_y(cx, cz + 2) == Some(ground) { world.set(cx, y, cz + 2, Block::OakLog); }
    if world.top_y(cx, cz - 2) == Some(ground) { world.set(cx, y, cz - 2, Block::OakLog); }
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
    
    // Árbol frontal removido a petición y reemplazado con bloque de coral luminoso elevado
    let ground_13_3 = world.top_y(13, 3).unwrap_or(0);
    world.set(13, ground_13_3 + 1, 3, Block::Fence);
    world.set(13, ground_13_3 + 2, 3, Block::Fence);
    world.set(13, ground_13_3 + 3, 3, Block::SeaLantern);
    world.add_light(Vec3::new(13.0, ground_13_3 as f32 + 3.0, 3.0), 0x88FFDD, 2.5, 20.0);

    // Solo dejamos el árbol del fondo a la izquierda (-3, 20)
    if let Some(top) = world.top_y(-3, 20) { build_tree(world, -3, 20, top + 1, 5, 2.4, Block::OakLog); }
    
    // Reemplazamos los otros árboles sueltos con pilares luminosos (Sea Lantern) súper fuertes
    if let Some(top) = world.top_y(24, 18) {
        world.set(24, top + 1, 18, Block::Fence);
        world.set(24, top + 2, 18, Block::Fence);
        world.set(24, top + 3, 18, Block::SeaLantern);
        world.add_light(Vec3::new(24.0, top as f32 + 3.0, 18.0), 0x88FFDD, 2.5, 20.0);
    }

    for (x, z, height) in [
        (5, 14, 2),
        (6, 14, 1),
        (6, 15, 1),
        (5, 15, 1),
        (4, 16, 1),
        (0, 8, 1),
        (-2, 4, 1),
        (1, 20, 1),
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
    let frame_z = back + 1;

    // Túnel horizontal hacia el precipicio
    for (x, wall) in cave_x.clone().zip(&walls) {
        for y in -5..=-2 {
            for z in frame_z..=*wall {
                world.remove(x, y, z);
            }
        }
    }

    // Fosa orgánica y natural tipo "cenote" inclinado
    for step in 0..=5 {
        let z = frame_z - 5 + step;
        let floor_y = -step; 
        
        // El camino serpentea suavemente usando una función seno
        let center_x = 12.5 + (step as f32 * 0.7).sin() * 1.5;
        
        for x in 7..=18 {
            let dx = (x as f32 - center_x).abs();
            // Ruido para bordes irregulares
            let noise = (hash(x * 7 + step, z * 3) % 100) as f32 / 100.0;
            
            let core_width = 1.2 + noise * 1.5;
            let slope_width = core_width + 3.0; // Anchura total incluyendo laderas

            if dx < slope_width {
                if let Some(top) = world.top_y(x, z) {
                    let mut slope_y = floor_y;
                    if dx > core_width {
                        slope_y += ((dx - core_width) * 1.5) as i32; // Sube gradualmente en las laderas
                    }

                    if slope_y <= top {
                        // Vaciamos el agujero
                        for y in slope_y..=top + 1 {
                            world.remove(x, y, z);
                        }

                        let rand_val = hash(x, z);
                        // Decoramos el terreno expuesto
                        if dx <= core_width {
                            // El fondo del camino se ve pisoteado y rocoso
                            let trail_block = match rand_val % 6 {
                                0 => Block::DirtPath,
                                1 => Block::Stone,
                                2 => Block::Rock,
                                _ => Block::Dirt,
                            };
                            world.set(x, slope_y - 1, z, trail_block);
                        } else {
                            // Las laderas tienen pasto y a veces piedra asomando
                            if rand_val % 4 == 0 {
                                world.set(x, slope_y - 1, z, Block::Stone);
                            } else {
                                world.set(x, slope_y - 1, z, Block::Grass);
                            }
                        }
                    }
                }
            }
        }
    }

    // Construcción del portal en la base de la fosa
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

    // Iluminación
    let front = frame_z as f32 + 0.6;
    let back_light = frame_z as f32 - 0.6;
    world.add_light(Vec3::new(9.0, -2.9, front), 0xFFAA50, 1.6, 6.0);
    
    // Luces del portal morado balanceadas
    world.add_light(Vec3::new(12.5, -3.5, front), 0xC222FF, 2.5, 10.0);
    world.add_light(Vec3::new(12.5, -3.5, back_light), 0xC222FF, 2.8, 15.0);

    // Al volver del Nether se aparece delante del portal, en la cornisa del precipicio.
    // Mira en diagonal para ver la pared de la cueva y no solo cielo.
    world.add_arrival(Realm::Nether, Vec3::new(12.5, -3.98, frame_z as f32 + 1.0), 0.7);
}

// Camara oculta del portal al End: una sala de ladrillo dentro de la roca, bajo
// el acantilado. No se ve desde arriba; solo se entra por un tunel que se abre
// en la pared oeste de la isla.
fn build_end_chamber(world: &mut World) {
    const FLOOR: i32 = -4;
    const CEILING: i32 = 1;
    let (x0, x1, z0, z1) = (-1, 6, 2, 10);

    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in FLOOR..=CEILING {
                let shell = x == x0 || x == x1 || z == z0 || z == z1 || y == FLOOR || y == CEILING;
                if shell {
                    world.set(x, y, z, Block::StoneBricks);
                } else {
                    world.remove(x, y, z);
                }
            }
        }
    }

    for x in -9..=x0 {
        for z in 5..=7 {
            for y in FLOOR + 1..=FLOOR + 3 {
                world.remove(x, y, z);
            }
            if world.get(x, FLOOR, z).is_some() {
                world.set(x, FLOOR, z, Block::StoneBricks);
            }
        }
    }

    // Marco de 5x5 sin esquinas, con la superficie del portal de 3x3 adentro.
    let (cx, cz) = (3, 6);
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            let on_ring = dx.abs() == 2 || dz.abs() == 2;
            if dx.abs() == 2 && dz.abs() == 2 {
                continue;
            }
            let block = if on_ring { Block::EndFrame } else { Block::EndPortal };
            world.set(cx + dx, FLOOR + 1, cz + dz, block);
        }
    }

    world.add_light(Vec3::new(cx as f32, -1.0, cz as f32), 0x5CFFD6, 1.8, 9.0);

    // Al volver del End se aparece junto al portal, mirando hacia el tunel de salida.
    world.add_arrival(
        Realm::End,
        Vec3::new(0.0, FLOOR as f32 + 2.02, cz as f32),
        std::f32::consts::PI,
    );
}

fn build_lanterns(world: &mut World) {
    // Un unico farol chico, en la punta del puente.
    let (x, z) = (16, 18);
    if let Some(ground) = world.top_y(x, z) {
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
        // El marco mide 13/16 de alto y la superficie del portal queda apenas por debajo.
        Block::EndFrame => vec![(Vec3::new(0.0, -0.09375, 0.0), Vec3::new(1.0, 0.8125, 1.0))],
        Block::EndPortal => vec![(Vec3::new(0.0, 0.22, 0.0), Vec3::new(1.0, 0.06, 1.0))],
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
        Block::Snowflake => vec![(Vec3::new(0.4, 0.4, 0.4), Vec3::new(0.15, 0.15, 0.15))],
        Block::Petal => vec![(Vec3::new(0.4, 0.4, 0.4), Vec3::new(0.2, 0.05, 0.2))],
        Block::Campfire => vec![(Vec3::new(0.0, -0.25, 0.0), Vec3::new(1.0, 0.5, 1.0))],
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
            cube.solid = block.blocks_movement();

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
fn build_particles(world: &mut World, biome: Biome) {
    let particle_block = match biome {
        Biome::Snow => Some(Block::Snowflake),
        Biome::Sakura => Some(Block::Petal),
        _ => None,
    };

    if let Some(block) = particle_block {
        for x in -5..=30 {
            for z in -5..=30 {
                // Cantidad drásticamente reducida (pocos) y solo en el piso
                if hash(x * 7, z * 13) % 25 == 0 {
                    if let Some(top) = world.top_y(x, z) {
                        // Lo colocamos justo encima del suelo (como si ya hubieran caído)
                        world.set_if_empty(x, top + 1, z, block);
                    }
                }
            }
        }
    }
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
    build_cliff_details(&mut world);
    build_end_chamber(&mut world);
    build_cabin_details(&mut world);
    build_clouds(&mut world);
    build_particles(&mut world, biome);
    build_ground_details(&mut world);
    build_camp(&mut world);

    // Arranca en la hora dorada del atardecer; el ciclo completo vive en daycycle.rs.
    let (sun, ambient, sky) = crate::daycycle::lighting_at(0.78);
    let mut lights = vec![sun];
    lights.append(&mut world.lights);

    let mut scene = Scene::new(to_cubes(&world, biome), textures, lights, ambient, sky, SKY_TEXTURE);

    // Cada bloque de portal es una zona de teletransporte del tamano de su celda.
    let half = Vec3::new(0.5, 0.5, 0.5);
    scene.portals = world
        .blocks
        .iter()
        .filter_map(|(&(x, y, z), &block)| {
            let destination = match block {
                Block::Portal => Realm::Nether,
                Block::EndPortal => Realm::End,
                _ => return None,
            };
            let center = Vec3::new(x as f32 - CENTER, y as f32, z as f32 - CENTER);
            Some(Portal { min: center - half, max: center + half, destination })
        })
        .collect();
    scene.arrivals = world.arrivals;

    scene
}
