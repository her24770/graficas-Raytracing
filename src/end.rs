use std::collections::HashMap;

use nalgebra_glm::Vec3;

use crate::color::Color;
use crate::cube::{face_index, face_tangent_axes, Cube};
use crate::light::Light;
use crate::ray_intersect::{FaceTextures, Material};
use crate::scene::{Ambient, Arrival, Portal, Realm, Scene, SkyGradient};
use crate::texture::Texture;

const TEXTURE_FILES: [&str; 12] = [
    "end_stone",
    "end_stone_bricks",
    "obsidian",
    "purpur_block",
    "purpur_pillar_side",
    "purpur_pillar_top",
    "chorus_plant",
    "sea_lantern",
    "end_portal_frame_top",
    "end_portal_frame_side",
    "end_portal",
    "sky_end",
];

const SKY_TEXTURE: usize = TEXTURE_FILES.len() - 1;

fn tex(name: &str) -> usize {
    TEXTURE_FILES
        .iter()
        .position(|file| *file == name)
        .unwrap_or_else(|| panic!("textura no registrada: {name}"))
}

const SIZE: i32 = 20;
const CENTER: f32 = 9.5;
const PAD: i32 = 3;
const RADIUS: f32 = 9.5 + PAD as f32;

const NEIGHBORS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    EndStone,
    EndStoneBricks,
    Obsidian,
    PurpurBlock,
    PurpurPillar,
    Chorus,
    Crystal,
    EndFrame,
    EndPortal,
}

impl Block {
    fn is_opaque_full(self) -> bool {
        !matches!(self, Block::EndFrame | Block::EndPortal)
    }

    fn appearance(self) -> (FaceTextures, Material) {
        const ROCK: Material = Material::matte(0.08, 10.0);
        const OBSIDIAN: Material = Material {
            reflectivity: 0.15,
            ..Material::matte(0.6, 80.0)
        };
        const PURPUR: Material = Material::matte(0.12, 20.0);
        const CRYSTAL: Material = Material {
            emission: 1.0,
            reflectivity: 0.1,
            ..Material::matte(0.3, 60.0)
        };

        let uniform = |name: &str| FaceTextures::uniform(tex(name));

        match self {
            Block::EndStone => (uniform("end_stone"), ROCK),
            Block::EndStoneBricks => (uniform("end_stone_bricks"), ROCK),
            Block::Obsidian => (uniform("obsidian"), OBSIDIAN),
            Block::PurpurBlock => (uniform("purpur_block"), PURPUR),
            Block::PurpurPillar => (
                FaceTextures::top_side_bottom(
                    tex("purpur_pillar_top"),
                    tex("purpur_pillar_side"),
                    tex("purpur_pillar_top"),
                ),
                PURPUR,
            ),
            Block::Chorus => (uniform("chorus_plant"), Material::matte(0.05, 8.0)),
            Block::Crystal => (uniform("sea_lantern"), CRYSTAL),
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

    fn add_light(&mut self, position: Vec3, color: u32, intensity: f32, range: f32) {
        let position = position - Vec3::new(CENTER, 0.0, CENTER);
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

    fn is_opaque(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z).is_some_and(Block::is_opaque_full)
    }

    fn top_y(&self, x: i32, z: i32) -> Option<i32> {
        (-10..=15).rev().find(|&y| self.get(x, y, z).is_some())
    }
}

fn hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (z as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

fn edge_distance(x: i32, z: i32) -> f32 {
    let dx = (x as f32 - CENTER) / RADIUS;
    let dz = (z as f32 - CENTER) / RADIUS;
    let wobble = 1.0 + 0.07 * ((4.0 * dz.atan2(dx)).sin());
    (dx.abs().powf(3.0) + dz.abs().powf(3.0)).powf(1.0 / 3.0) / wobble
}

// Terreno chato y liso, como las islas reales del End: sin rio, sin cueva,
// solo piedra del End con un nucleo de piedra bricks y obsidiana en el fondo.
fn build_terrain(world: &mut World) {
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            let q = edge_distance(x, z);
            if q > 1.0 {
                continue;
            }

            let top = (hash(x, z) % 2) as i32;
            let depth = 4.0 + 4.0 * (1.0 - q).max(0.0).powf(0.7);
            let bottom = -(depth.round() as i32);
            let obsidian_line = bottom + 1;

            for y in bottom..=top {
                let block = if y <= obsidian_line {
                    Block::Obsidian
                } else if y == top {
                    Block::EndStone
                } else {
                    Block::EndStoneBricks
                };
                world.set(x, y, z, block);
            }
        }
    }
}

// Las columnas de obsidiana con su cristal brillante arriba: la jaula que
// rodea al dragon en el juego real.
fn build_crystal_pillars(world: &mut World) {
    let pillars: [(i32, i32, i32); 5] = [(2, 10, 9), (15, 12, 3), (-1, 8, 14), (17, 9, 16), (8, 14, -1)];

    for (x, z, height) in pillars {
        let Some(base) = world.top_y(x, z) else {
            continue;
        };
        for y in base + 1..=base + height {
            world.set(x, y, z, Block::Obsidian);
        }
        world.set(x, base + height + 1, z, Block::Crystal);
        world.add_light(
            Vec3::new(x as f32, base as f32 + height as f32 + 1.3, z as f32),
            0xB8F5E8,
            2.0,
            10.0,
        );
    }
}

// Plantas de corus: crecen hacia arriba con giros al azar, no en linea recta.
fn build_chorus(world: &mut World, x: i32, z: i32, height: i32) {
    let Some(base) = world.top_y(x, z) else {
        return;
    };

    let mut cx = x;
    let mut cz = z;
    for h in 1..=height {
        world.set_if_empty(cx, base + h, cz, Block::Chorus);
        match hash(cx * 3 + h * 7, cz * 5 + h * 11) % 5 {
            0 => cx += 1,
            1 => cx -= 1,
            2 => cz += 1,
            3 => cz -= 1,
            _ => {}
        }
    }
}

fn build_chorus_grove(world: &mut World) {
    let sites: [(i32, i32, i32); 6] = [
        (5, 5, 6),
        (12, 16, 5),
        (18, 10, 4),
        (3, 15, 7),
        (10, 3, 5),
        (16, 18, 6),
    ];
    for (x, z, height) in sites {
        build_chorus(world, x, z, height);
    }
}

// Fragmento de ciudad del End: dos torres de purpur de distinta altura
// conectadas por un puentecito, como los que aparecen en las ciudades reales.
fn build_end_city(world: &mut World) {
    let (x, z) = (12, 8);
    let Some(base) = world.top_y(x, z) else {
        return;
    };

    for y in base + 1..=base + 8 {
        world.set(x, y, z, Block::PurpurPillar);
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            world.set(x + dx, base + 8, z + dz, Block::PurpurBlock);
        }
    }

    let (x2, z2) = (x + 4, z);
    for y in base + 1..=base + 5 {
        world.set(x2, y, z2, Block::PurpurPillar);
    }
    for dx in -1..=1 {
        world.set(x2 + dx, base + 5, z2, Block::PurpurBlock);
    }

    for bridge_x in x + 1..x2 {
        world.set(bridge_x, base + 5, z, Block::EndStoneBricks);
    }
}

// Portal de regreso al overworld: el mismo marco con ojos que hay en la camara
// oculta de la isla, sobre un claro nivelado. Devuelve el punto de llegada.
fn build_exit_portal(world: &mut World) -> Arrival {
    const FLOOR: i32 = 1;
    let (cx, cz) = (7, 12);

    for x in cx - 3..=cx + 3 {
        for z in cz - 5..=cz + 3 {
            if world.top_y(x, z).is_none() {
                continue;
            }
            world.set(x, FLOOR, z, Block::EndStone);
            for y in FLOOR + 1..=FLOOR + 6 {
                world.blocks.remove(&(x, y, z));
            }
        }
    }

    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() == 2 && dz.abs() == 2 {
                continue;
            }
            let on_ring = dx.abs() == 2 || dz.abs() == 2;
            let block = if on_ring { Block::EndFrame } else { Block::EndPortal };
            world.set(cx + dx, FLOOR + 1, cz + dz, block);
        }
    }
    world.add_light(Vec3::new(cx as f32, FLOOR as f32 + 2.5, cz as f32), 0x5CFFD6, 1.8, 9.0);

    // Se aparece a dos bloques del marco, mirando hacia el portal de regreso.
    Arrival {
        from: Realm::Overworld,
        eye: Vec3::new(cx as f32 - CENTER, FLOOR as f32 + 2.02, (cz - 4) as f32 - CENTER),
        yaw: std::f32::consts::FRAC_PI_2,
    }
}

const OCCLUSION_LEVELS: [f32; 4] = [1.0, 0.78, 0.6, 0.45];

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

fn to_cubes(world: &World) -> Vec<Cube> {
    let mut cubes = Vec::new();

    for (&(x, y, z), &block) in &world.blocks {
        let exposed = NEIGHBORS
            .iter()
            .any(|&(dx, dy, dz)| !world.is_opaque(x + dx, y + dy, z + dz));
        if !exposed {
            continue;
        }

        let (textures, material) = block.appearance();
        let center = Vec3::new(x as f32 - CENTER, y as f32, z as f32 - CENTER);
        // El marco mide 13/16 de alto y la superficie del portal es una lamina fina.
        let (offset, size) = match block {
            Block::EndFrame => (Vec3::new(0.0, -0.09375, 0.0), Vec3::new(1.0, 0.8125, 1.0)),
            Block::EndPortal => (Vec3::new(0.0, 0.22, 0.0), Vec3::new(1.0, 0.06, 1.0)),
            _ => (Vec3::zeros(), Vec3::new(1.0, 1.0, 1.0)),
        };
        let mut cube = Cube::new(center + offset, size, material, textures);
        cube.solid = block != Block::EndPortal;

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

    cubes
}

pub fn build_end() -> Scene {
    let textures = TEXTURE_FILES
        .iter()
        .map(|name| Texture::from_bmp(&format!("assets/textures/{name}.bmp")))
        .collect();

    let mut world = World::new();
    build_terrain(&mut world);
    build_crystal_pillars(&mut world);
    build_end_city(&mut world);
    build_chorus_grove(&mut world);
    let arrival = build_exit_portal(&mut world);

    // Sin sol: una luz muy tenue y fria desde arriba, mas los cristales de
    // las columnas y una ambiental palida y calma, como el vacio del End.
    let dim_light = Light::new(Vec3::new(0.0, 1000.0, 0.0), Color::from_hex(0x8A80B0), 0.4, f32::INFINITY);
    let mut lights = vec![dim_light];
    lights.append(&mut world.lights);

    let ambient = Ambient {
        sky: Color::from_hex(0x8A7EB8),
        ground: Color::from_hex(0x453A66),
        intensity: 0.6,
    };

    // Tinte claro: la textura del cielo ya aporta el negro del vacio y las
    // estrellas, este color solo lo multiplica (tenir), no debe ser casi negro.
    let sky = SkyGradient {
        horizon: Color::from_hex(0xC8BEE8),
        middle: Color::from_hex(0x9A8CC8),
        high: Color::from_hex(0x6858A0),
    };

    let mut scene = Scene::new(to_cubes(&world), textures, lights, ambient, sky, SKY_TEXTURE);

    let half = Vec3::new(0.5, 0.5, 0.5);
    scene.portals = world
        .blocks
        .iter()
        .filter(|&(_, &block)| block == Block::EndPortal)
        .map(|(&(x, y, z), _)| {
            let center = Vec3::new(x as f32 - CENTER, y as f32, z as f32 - CENTER);
            Portal { min: center - half, max: center + half, destination: Realm::Overworld }
        })
        .collect();
    scene.arrivals = vec![arrival];

    scene
}
