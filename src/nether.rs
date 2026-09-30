use std::collections::HashMap;

use nalgebra_glm::Vec3;

use crate::color::Color;
use crate::cube::{face_index, face_tangent_axes, Cube};
use crate::light::Light;
use crate::ray_intersect::{FaceTextures, Material};
use crate::scene::{Ambient, Scene, SkyGradient};
use crate::texture::Texture;

const TEXTURE_FILES: [&str; 12] = [
    "netherrack",
    "nether_bricks",
    "soul_sand",
    "magma",
    "lava_still",
    "crimson_nylium",
    "crimson_nylium_side",
    "crimson_stem",
    "crimson_stem_top",
    "nether_wart_block",
    "shroomlight",
    "obsidian",
];

fn tex(name: &str) -> usize {
    TEXTURE_FILES
        .iter()
        .position(|file| *file == name)
        .unwrap_or_else(|| panic!("textura no registrada: {name}"))
}

const SIZE: i32 = 22;
const CENTER: f32 = 10.5;
const PAD: i32 = 3;
const RADIUS: f32 = 10.5 + PAD as f32;

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
    Netherrack,
    SoulSand,
    CrimsonNylium,
    Obsidian,
    Lava,
    NetherBricks,
    CrimsonStem,
    NetherWartBlock,
    Glowstone,
    Shroomlight,
}

impl Block {
    fn is_opaque_full(self) -> bool {
        !matches!(self, Block::Lava)
    }

    fn appearance(self) -> (FaceTextures, Material) {
        const ROCK: Material = Material::matte(0.1, 12.0);
        const LAVA: Material = Material {
            diffuse: 0.6,
            emission: 0.55,
            animated: true,
            ..Material::matte(0.3, 40.0)
        };
        const OBSIDIAN: Material = Material {
            reflectivity: 0.15,
            ..Material::matte(0.6, 80.0)
        };
        const GLOW: Material = Material {
            emission: 1.0,
            ..Material::matte(0.0, 1.0)
        };

        let uniform = |name: &str| FaceTextures::uniform(tex(name));

        match self {
            Block::Netherrack => (uniform("netherrack"), ROCK),
            Block::SoulSand => (uniform("soul_sand"), ROCK),
            Block::CrimsonNylium => (
                FaceTextures::top_side_bottom(
                    tex("crimson_nylium"),
                    tex("crimson_nylium_side"),
                    tex("netherrack"),
                ),
                ROCK,
            ),
            Block::Obsidian => (uniform("obsidian"), OBSIDIAN),
            Block::Lava => (uniform("lava_still"), LAVA),
            Block::NetherBricks => (uniform("nether_bricks"), ROCK),
            Block::CrimsonStem => (
                FaceTextures::top_side_bottom(
                    tex("crimson_stem_top"),
                    tex("crimson_stem"),
                    tex("crimson_stem_top"),
                ),
                Material::matte(0.08, 10.0),
            ),
            Block::NetherWartBlock => (uniform("nether_wart_block"), Material::matte(0.05, 6.0)),
            Block::Glowstone | Block::Shroomlight => {
                let name = if matches!(self, Block::Shroomlight) {
                    "shroomlight"
                } else {
                    "nether_wart_block"
                };
                let _ = name;
                (uniform("shroomlight"), GLOW)
            }
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
    let wobble = 1.0 + 0.08 * ((3.0 * dz.atan2(dx)).sin());
    (dx.abs().powf(3.0) + dz.abs().powf(3.0)).powf(1.0 / 3.0) / wobble
}

fn build_terrain(world: &mut World) {
    for x in -PAD..SIZE + PAD {
        for z in -PAD..SIZE + PAD {
            let q = edge_distance(x, z);
            if q > 1.0 {
                continue;
            }

            let top_jitter = (hash(x, z) % 3) as i32 - 1;
            let top = 1 + top_jitter.max(0);
            let depth = 5.0 + 5.0 * (1.0 - q).max(0.0).powf(0.7);
            let bottom = -(depth.round() as i32);
            let obsidian_line = bottom + 2 + (hash(x + 4, z) % 2) as i32;

            for y in bottom..=top {
                let block = if y == top {
                    match hash(x + 11, z + 5) % 10 {
                        0 => Block::SoulSand,
                        1 | 2 => Block::CrimsonNylium,
                        _ => Block::Netherrack,
                    }
                } else if y > obsidian_line {
                    Block::Netherrack
                } else {
                    Block::Obsidian
                };
                world.set(x, y, z, block);
            }
        }
    }
}

// Rio de lava serpenteante (no una franja recta) que cae por el borde de la isla.
fn build_lava(world: &mut World) {
    let mut last_lava: Option<(i32, i32)> = None;

    for x in -PAD..SIZE + PAD {
        let wander = ((x as f32 * 0.32).sin() * 2.3 + (x as f32 * 0.11).sin() * 1.4).round() as i32;
        let center_z = 9 + wander;
        let width = 1 + (hash(x, 100) % 2) as i32;

        for z in center_z - width..=center_z + width {
            if world.get(x, 1, z).is_some() {
                for y in 1..=2 {
                    world.set(x, y, z, Block::Lava);
                }
                last_lava = Some((x, z));
            }
        }
    }

    if let Some((edge_x, edge_z)) = last_lava {
        for y in -8..=1 {
            world.set(edge_x + 1, y, edge_z, Block::Lava);
        }
    }
}

// Pilares oscuros tipo basalto, subiendo bien alto sobre el terreno. Densos y
// de altura variada, para que se vea como una garganta llena de columnas.
fn build_pillars(world: &mut World) {
    let pillars: [(i32, i32, i32); 16] = [
        (2, 4, 14),
        (17, 6, 3),
        (19, 5, 15),
        (-1, 7, 5),
        (12, 5, -1),
        (21, 4, 9),
        (8, 2, 18),
        (14, 9, 0),
        (-2, 3, 12),
        (22, 6, 4),
        (6, 8, 3),
        (18, 4, 19),
        (0, 5, -2),
        (24, 3, 15),
        (10, 6, 20),
        (16, 3, -3),
    ];

    for (x, z, height) in pillars {
        let Some(base) = world.top_y(x, z) else {
            continue;
        };
        for y in base + 1..=base + height {
            world.set(x, y, z, Block::Obsidian);
        }
    }
}

// Picos y montañas de netherrack que suben bien alto, para romper la silueta
// chata y que la isla se vea como una garganta escarpada, no una mesa plana.
fn build_spires(world: &mut World) {
    let spires: [(i32, i32, i32); 4] = [(9, 11, 11), (5, 16, 7), (17, 7, 9), (1, 3, 6)];

    for (x, z, height) in spires {
        let Some(base) = world.top_y(x, z) else {
            continue;
        };
        for h in 1..=height {
            let radius = (((height - h) as f32 / height as f32) * 2.6).round() as i32;
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx * dx + dz * dz > radius * radius {
                        continue;
                    }
                    let block = if hash(x + dx * 7 + h, z + dz * 11) % 7 == 0 {
                        Block::Obsidian
                    } else {
                        Block::Netherrack
                    };
                    world.set_if_empty(x + dx, base + h, z + dz, block);
                }
            }
        }
    }
}

// Ruina de ladrillo del Nether: paredes con huecos, sin techo completo.
fn build_ruin(world: &mut World) {
    let (x0, x1, z0, z1) = (12, 18, 12, 17);
    let base = 1;

    for y in base..=base + 4 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let on_wall = x == x0 || x == x1 || z == z0 || z == z1;
                if !on_wall {
                    continue;
                }
                // Huecos tipo ventana, y una abertura para la entrada.
                let is_gap = (y == base + 2 && (x + z) % 3 == 0) || (x == x0 && z == z0 + 2 && y <= base + 1);
                if is_gap {
                    continue;
                }
                world.set(x, y, z, Block::NetherBricks);
            }
        }
    }

    // Techo parcial, roto.
    for x in x0..=x1 {
        for z in z0..=z1 {
            if hash(x, z) % 5 != 0 {
                world.set_if_empty(x, base + 5, z, Block::NetherBricks);
            }
        }
    }

    // Escalinata interior.
    for (i, y) in (base..=base + 3).enumerate() {
        world.set(x0 + 1, y, z0 + 1 + i as i32, Block::Obsidian);
    }

    // Antorchas de glowstone en las paredes.
    world.set(x0, base + 3, (z0 + z1) / 2, Block::Glowstone);
    world.set(x1, base + 3, (z0 + z1) / 2, Block::Glowstone);
    world.add_light(
        Vec3::new(x0 as f32, base as f32 + 2.8, ((z0 + z1) / 2) as f32),
        0xFF8A3D,
        1.8,
        9.0,
    );
    world.add_light(
        Vec3::new(x1 as f32, base as f32 + 2.8, ((z0 + z1) / 2) as f32),
        0xFF8A3D,
        1.8,
        9.0,
    );
}

// Hongos gigantes: tronco de crimson_stem con una copa de nether_wart_block.
fn build_fungus(world: &mut World, x: i32, z: i32, height: i32) {
    let Some(base) = world.top_y(x, z) else {
        return;
    };

    for y in base + 1..=base + height {
        world.set(x, y, z, Block::CrimsonStem);
    }

    let top = base + height;
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            if hash(x + dx * 5, z + dz * 7) % 3 != 0 {
                world.set_if_empty(x + dx, top, z + dz, Block::NetherWartBlock);
            }
        }
    }
    world.set_if_empty(x, top, z, Block::NetherWartBlock);
}

// Racimos de glowstone y shroomlight incrustados, y verrugas sueltas en el piso.
fn build_glow_details(world: &mut World) {
    let clusters: [(i32, i32, i32, Block); 14] = [
        (4, 3, 4, Block::Glowstone),
        (20, 2, 18, Block::Shroomlight),
        (0, 6, 16, Block::Glowstone),
        (15, 8, 4, Block::Shroomlight),
        (9, 1, -2, Block::Glowstone),
        (2, 6, 15, Block::Shroomlight),
        (18, 8, 5, Block::Glowstone),
        (21, 2, 10, Block::Shroomlight),
        (-1, 9, 6, Block::Glowstone),
        (7, 2, 19, Block::Shroomlight),
        (13, 11, 1, Block::Glowstone),
        (5, 18, 8, Block::Shroomlight),
        (16, 5, 20, Block::Glowstone),
        (23, 5, 16, Block::Shroomlight),
    ];

    for (x, y, z, block) in clusters {
        world.set(x, y, z, block);
        let color = if block == Block::Shroomlight { 0xFF7A3D } else { 0xFFC97A };
        world.add_light(Vec3::new(x as f32, y as f32, z as f32), color, 1.7, 8.0);
    }

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
        if world.get(x, top + 1, z).is_some() {
            continue;
        }
        if matches!(world.get(x, top, z), Some(Block::NetherBricks) | Some(Block::Lava)) {
            continue;
        }
        if hash(x + 21, z + 33) % 6 == 0 {
            world.set(x, top + 1, z, Block::NetherWartBlock);
        }
    }
}

// Rocas de netherrack y obsidiana cayendose bajo la isla, como en el overworld.
fn build_floating_debris(world: &mut World) {
    let clusters: [(i32, i32, i32); 5] = [
        (4, -12, 9),
        (18, -11, 3),
        (0, -13, 17),
        (21, -10, 14),
        (11, -14, 21),
    ];

    for (cx, cy, cz) in clusters {
        for (dx, dy, dz) in [(0i32, 0i32, 0i32), (1, 0, 0), (0, 0, 1), (0, -1, 0)] {
            let block = if (dx + dy + dz).abs() % 2 == 0 {
                Block::Netherrack
            } else {
                Block::Obsidian
            };
            world.set(cx + dx, cy + dy, cz + dz, block);
        }
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
        let mut cube = Cube::new(center, Vec3::new(1.0, 1.0, 1.0), material, textures);

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

pub fn build_nether() -> Scene {
    let textures = TEXTURE_FILES
        .iter()
        .map(|name| Texture::from_bmp(&format!("assets/textures/{name}.bmp")))
        .collect();

    let mut world = World::new();
    build_terrain(&mut world);
    build_lava(&mut world);
    build_spires(&mut world);
    build_pillars(&mut world);
    build_ruin(&mut world);
    build_fungus(&mut world, 5, 17, 5);
    build_fungus(&mut world, 3, 6, 4);
    build_fungus(&mut world, 20, 12, 4);
    build_glow_details(&mut world);
    build_floating_debris(&mut world);

    // Sin sol ni ciclo de dia: una luz muy tenue desde arriba mas el brillo
    // ambiente de la propia roca son lo que revela el terreno a la distancia;
    // la lava, glowstone y shroomlight aportan los puntos de luz fuerte.
    let dim_sun = Light::new(Vec3::new(0.0, 1000.0, 0.0), Color::from_hex(0x5A3020), 0.5, f32::INFINITY);
    let mut lights = vec![dim_sun];
    lights.append(&mut world.lights);

    let ambient = Ambient {
        sky: Color::from_hex(0x4A2416),
        ground: Color::from_hex(0x5C2C14),
        intensity: 0.85,
    };

    let sky = SkyGradient {
        horizon: Color::from_hex(0x120404),
        middle: Color::from_hex(0x0A0202),
        high: Color::from_hex(0x000000),
    };

    Scene::new(to_cubes(&world), textures, lights, ambient, sky)
}
