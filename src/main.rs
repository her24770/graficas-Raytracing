mod audio;
mod camera;
mod color;
mod cube;
mod daycycle;
mod diorama;
mod end;
mod framebuffer;
mod light;
mod nether;
mod player;
mod ray_intersect;
mod scene;
mod texture;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use nalgebra_glm::{dot, normalize, Vec3};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::audio::Music;
use crate::camera::Camera;
use crate::color::Color;
use crate::framebuffer::Framebuffer;
use crate::player::{Controls, Player};
use crate::ray_intersect::Intersect;
use crate::scene::{Realm, Scene};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const FOV: f32 = PI / 4.0;
// En primera persona hace falta un campo de vision mas abierto, como el de Minecraft.
const FREE_FOV: f32 = 70.0 * PI / 180.0;
const ROTATION_SPEED: f32 = PI / 60.0;
const ZOOM_SPEED: f32 = 0.985;

const SHADOW_BIAS: f32 = 1e-3;
const MAX_SHADOW_CROSSINGS: usize = 6;

const REFLECTION_BIAS: f32 = 1e-3;
const MAX_DEPTH: u32 = 3;
const MUSIC_FILE: &str = "assets/audio/musica.mp3";

// Fraccion del dia completo que avanza cada tick del bucle principal.
const AUTO_TIME_STEP: f32 = 0.00035;
const MANUAL_TIME_STEP: f32 = 0.003;
// Velocidades del ciclo de dia (tecla V): multiplican el avance automatico de la hora.
const TIME_SPEEDS: [(f32, &str); 3] = [(1.0, "normal"), (4.0, "rapido"), (12.0, "muy rapido")];
const START_TIME_OF_DAY: f32 = 0.78;

// Degradado vertical usado como tinte (atardecer, bioma, etc.) sobre la textura del cielo.
fn sky_tint(ray_direction: &Vec3, sky: &crate::scene::SkyGradient) -> Color {
    let t = ((ray_direction.y + 1.0) / 1.2).clamp(0.0, 1.0);
    if t < 0.5 {
        Color::lerp(sky.horizon, sky.middle, t / 0.5)
    } else {
        Color::lerp(sky.middle, sky.high, (t - 0.5) / 0.5)
    }
}

// Mapeo equirectangular: convierte la direccion 3D del rayo en coordenadas (u, v)
// de una textura panoramica, para que el cielo envuelva la escena en todas direcciones.
fn sky_color(ray_direction: &Vec3, scene: &Scene) -> Color {
    let u = (ray_direction.z.atan2(ray_direction.x) / (2.0 * PI)) + 0.5;
    let v = ray_direction.y.clamp(-1.0, 1.0).acos() / PI;

    let texture = &scene.textures[scene.sky_texture];
    let sample = texture.sample(u, v);
    let tint = sky_tint(ray_direction, &scene.sky);

    // El cielo estaba muy cargado: usamos el gradiente limpio como base principal (85%)
    // y la textura aporta solo un detalle muy sutil (15%) para que no sature la vista.
    let base = tint * 0.85 + sample.modulate(tint) * 0.15;
    
    // Filtramos las estrellas para que solo los pixeles EXTREMADAMENTE blancos brillen,
    // y lo hagan de forma tenue, para evitar que parezca ruido.
    let luma = (sample.r as f32 + sample.g as f32 + sample.b as f32) / (255.0 * 3.0);
    let star_glow = luma.powi(16) * 0.3; // powi(16) mata los grises, solo sobrevive el blanco puro
    
    base + sample * star_glow
}

pub fn reflect(incident: &Vec3, normal: &Vec3) -> Vec3 {
    incident - normal * (2.0 * dot(incident, normal))
}

// Ley de Snell. None si el angulo es tan cerrado que el rayo no puede salir
// del material (reflexion interna total): ahi todo se refleja, nada se transmite.
fn refract(incident: &Vec3, normal: &Vec3, refractive_index: f32) -> Option<Vec3> {
    let mut cos_i = dot(incident, normal).clamp(-1.0, 1.0);
    let mut n = *normal;
    let mut eta = 1.0 / refractive_index;

    if cos_i > 0.0 {
        // El rayo ya esta dentro del material y busca salir.
        n = -normal;
        eta = refractive_index;
    } else {
        cos_i = -cos_i;
    }

    let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
    if k < 0.0 {
        None
    } else {
        Some(incident * eta + n * (eta * cos_i - k.sqrt()))
    }
}

// Aproximacion de Schlick: que fraccion de la luz se refleja segun el angulo,
// en vez de una mezcla fija. A angulo rasante casi todo se refleja, de frente casi nada.
fn fresnel_reflectance(incident: &Vec3, normal: &Vec3, refractive_index: f32) -> f32 {
    let cos_i = dot(incident, normal).clamp(-1.0, 1.0).abs();
    let r0 = ((1.0 - refractive_index) / (1.0 + refractive_index)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cos_i).powi(5)
}

// Fraccion de luz que llega desde la luz al punto: 0 en sombra, 1 sin obstaculos.
// Los bloques emisivos no proyectan sombra y los transparentes dejan pasar parte de la luz.
fn light_visibility(intersect: &Intersect, light_direction: &Vec3, light_distance: f32, scene: &Scene) -> f32 {
    let mut origin = intersect.point + intersect.normal * SHADOW_BIAS;
    let mut traveled = 0.0;
    let mut visibility = 1.0;

    for _ in 0..MAX_SHADOW_CROSSINGS {
        let Some(blocker) = scene.trace(&origin, light_direction) else {
            return visibility;
        };

        traveled += blocker.distance;
        if traveled >= light_distance {
            return visibility;
        }

        let material = blocker.material;
        if material.emission <= 0.0 {
            if material.transparency <= 0.0 {
                return 0.0;
            }
            visibility *= material.transparency;
        }

        origin = blocker.point + light_direction * SHADOW_BIAS;
        traveled += SHADOW_BIAS;
    }

    visibility
}

fn shade(intersect: &Intersect, ray_origin: &Vec3, scene: &Scene, time: f32) -> Color {
    let material = intersect.material;
    let (u, v) = if material.animated {
        (intersect.u + time * 0.06, intersect.v + time * 0.035)
    } else {
        (intersect.u, intersect.v)
    };
    let base = scene.textures[intersect.texture_id].sample(u, v).to_vec3();
    let view_direction = (ray_origin - intersect.point).normalize();

    let hemisphere = intersect.normal.y * 0.5 + 0.5;
    let ambient = scene.ambient.ground.to_vec3().lerp(&scene.ambient.sky.to_vec3(), hemisphere)
        * scene.ambient.intensity;

    let mut diffuse = Vec3::zeros();
    let mut specular = Vec3::zeros();

    for light in &scene.lights {
        let to_light = light.position - intersect.point;
        let light_distance = to_light.magnitude();
        if light_distance >= light.range {
            continue;
        }

        let light_direction = to_light / light_distance;
        let lambert = dot(&intersect.normal, &light_direction);
        if lambert <= 0.0 {
            continue;
        }

        let strength = light.intensity
            * light.attenuation(light_distance)
            * light_visibility(intersect, &light_direction, light_distance, scene);
        if strength <= 0.0 {
            continue;
        }

        let light_color = light.color.to_vec3() * strength;
        diffuse += light_color * lambert;

        let reflect_direction = reflect(&-light_direction, &intersect.normal);
        let highlight = dot(&view_direction, &reflect_direction)
            .max(0.0)
            .powf(material.specular_exponent);
        specular += light_color * (highlight * material.specular);
    }

    let lit = (ambient + diffuse * material.diffuse) * intersect.ambient_occlusion;
    let color = base.component_mul(&lit) + specular + base * material.emission;

    Color::from_vec3(color)
}

fn cast_ray(ray_origin: &Vec3, ray_direction: &Vec3, scene: &Scene, time: f32, depth: u32) -> Color {
    let Some(intersect) = scene.trace(ray_origin, ray_direction) else {
        return sky_color(ray_direction, scene);
    };

    let color = shade(&intersect, ray_origin, scene, time);
    let material = intersect.material;

    if depth >= MAX_DEPTH {
        return color;
    }

    if material.transparency > 0.0 {
        let reflect_direction = reflect(ray_direction, &intersect.normal).normalize();
        let reflect_origin = intersect.point + intersect.normal * REFLECTION_BIAS;
        let reflected = cast_ray(&reflect_origin, &reflect_direction, scene, time, depth + 1);

        let transmission = match refract(ray_direction, &intersect.normal, material.refractive_index) {
            Some(refract_direction) => {
                let refract_direction = refract_direction.normalize();
                let refract_origin = intersect.point + refract_direction * REFLECTION_BIAS;
                let refracted = cast_ray(&refract_origin, &refract_direction, scene, time, depth + 1);

                let fresnel = fresnel_reflectance(ray_direction, &intersect.normal, material.refractive_index);
                reflected * fresnel + refracted * (1.0 - fresnel)
            }
            // Reflexion interna total: el angulo es tan cerrado que no hay salida posible.
            None => reflected,
        };

        return color * (1.0 - material.transparency) + transmission * material.transparency;
    }

    if material.reflectivity > 0.0 {
        let reflect_direction = reflect(ray_direction, &intersect.normal).normalize();
        let reflect_origin = intersect.point + intersect.normal * REFLECTION_BIAS;
        let reflected = cast_ray(&reflect_origin, &reflect_direction, scene, time, depth + 1);
        return color * (1.0 - material.reflectivity) + reflected * material.reflectivity;
    }

    color
}

fn render_band(
    band: &mut [u32],
    y_offset: usize,
    width: usize,
    height: usize,
    scene: &Scene,
    camera: &Camera,
    fov: f32,
    time: f32,
) {
    let width_f = width as f32;
    let height_f = height as f32;
    let aspect_ratio = width_f / height_f;
    let perspective_scale = (fov / 2.0).tan();

    let band_height = band.len() / width;

    for local_y in 0..band_height {
        let y = y_offset + local_y;

        for x in 0..width {
            let screen_x = (2.0 * x as f32) / width_f - 1.0;
            let screen_y = -(2.0 * y as f32) / height_f + 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = normalize(&Vec3::new(screen_x, screen_y, -1.0));
            let ray_direction = camera.basis_change(&ray_direction);

            band[local_y * width + x] =
                cast_ray(&camera.eye, &ray_direction, scene, time, 0).to_hex();
        }
    }
}

fn render(framebuffer: &mut Framebuffer, scene: &Scene, camera: &Camera, fov: f32, time: f32) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    let threads = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4);
    let rows_per_band = height.div_ceil(threads);

    std::thread::scope(|scope| {
        for (index, band) in framebuffer
            .buffer
            .chunks_mut(rows_per_band * width)
            .enumerate()
        {
            let y_offset = index * rows_per_band;
            scope.spawn(move || {
                render_band(band, y_offset, width, height, scene, camera, fov, time);
            });
        }
    });
}

fn apply_biome_mood(sun: &mut light::Light, ambient: &mut scene::Ambient, sky: &mut scene::SkyGradient, biome: diorama::Biome) {
    let (accent, blend, intensity_scale) = biome.mood();
    sun.color = Color::lerp(sun.color, accent, blend * 0.5);
    sun.intensity *= intensity_scale;
    ambient.sky = Color::lerp(ambient.sky, accent, blend);
    ambient.ground = Color::lerp(ambient.ground, accent, blend);
    sky.horizon = Color::lerp(sky.horizon, accent, blend);
    sky.middle = Color::lerp(sky.middle, accent, blend);
    sky.high = Color::lerp(sky.high, accent, blend);
}

fn build_realm(realm: Realm, biome: diorama::Biome) -> Scene {
    match realm {
        Realm::Overworld => diorama::build_diorama(biome),
        Realm::Nether => nether::build_nether(),
        Realm::End => end::build_end(),
    }
}

fn main() {
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut current_biome = diorama::Biome::Overworld;
    let mut scene = diorama::build_diorama(current_biome);

    let mut camera = Camera::new(
        Vec3::new(27.5, 21.0, 27.5),
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );

    // Modo captura: `cargo run --release -- --screenshot salida.bmp [giro_en_pasos] [hora_0_a_1] [bioma_0_a_5_o_6_nether_o_7_end]`
    let mut realm = Realm::Overworld;
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--screenshot" {
        if let Some(steps) = args.get(3).and_then(|value| value.parse::<i32>().ok()) {
            camera.orbit(steps as f32 * ROTATION_SPEED, 0.0);
        }
        if let Some(index) = args.get(5).and_then(|value| value.parse::<usize>().ok()) {
            if index == 6 {
                realm = Realm::Nether;
                scene = nether::build_nether();
            } else if index == 7 {
                realm = Realm::End;
                scene = end::build_end();
            } else {
                // Mismo orden que las teclas 1-6: los biomas de la isla, con Sakura al final.
                current_biome = diorama::Biome::ALL[index.min(5)];
                scene = diorama::build_diorama(current_biome);
            }
        }
        if realm == Realm::Overworld {
            if let Some(time_of_day) = args.get(4).and_then(|value| value.parse::<f32>().ok()) {
                let (mut sun, mut ambient, mut sky) = daycycle::lighting_at(time_of_day);
                apply_biome_mood(&mut sun, &mut ambient, &mut sky, current_biome);
                scene.lights[0] = sun;
                scene.ambient = ambient;
                scene.sky = sky;
            }
        }
        let start = Instant::now();
        render(&mut framebuffer, &scene, &camera, FOV, 0.0);
        println!("{} cubos, render en {:?}", scene.cubes.len(), start.elapsed());
        framebuffer
            .save_bmp(&args[2])
            .expect("no se pudo guardar la captura");
        return;
    }

    let mut window = Window::new("Diorama Raytracing", WIDTH, HEIGHT, WindowOptions::default())
        .expect("no se pudo crear la ventana");

    let frame_delay = Duration::from_millis(16);

    let mut time_of_day = START_TIME_OF_DAY;
    let mut auto_play = true;
    let mut time_speed = 0;
    let clock = Instant::now();

    // None = camara orbital (modo 1); Some = camara libre con colision (modo 2).
    let mut player: Option<Player> = None;
    let mut last_frame = Instant::now();
    let mut music = Music::start(MUSIC_FILE);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // El movimiento libre usa tiempo real; el tope evita saltos si un cuadro tarda mucho.
        let dt = last_frame.elapsed().as_secs_f32().min(0.1);
        last_frame = Instant::now();

        music.keep_looping();
        if window.is_key_pressed(Key::M, KeyRepeat::No) {
            music.toggle();
            println!("musica: {}", if music.is_playing() { "encendida" } else { "apagada" });
        }

        if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
            player = match player {
                Some(_) => None,
                None => Some(Player::from_camera(&camera, &scene)),
            };
            println!("camara: {}", if player.is_some() { "libre" } else { "orbital" });
        }

        let axis = |positive: Key, negative: Key| {
            window.is_key_down(positive) as i32 as f32 - window.is_key_down(negative) as i32 as f32
        };

        if let Some(player) = &mut player {
            if window.is_key_pressed(Key::G, KeyRepeat::No) {
                player.toggle_walking();
                println!("camara libre: {}", if player.is_walking() { "caminar" } else { "volar" });
            }
            let controls = Controls {
                forward: axis(Key::W, Key::S),
                strafe: axis(Key::D, Key::A),
                vertical: axis(Key::Space, Key::LeftShift),
                turn: axis(Key::Right, Key::Left),
                look_up: axis(Key::Up, Key::Down),
                jump: window.is_key_down(Key::Space),
            };
            player.update(&scene, controls, dt);

            // Tocar un portal teletransporta: se carga el otro mundo y el jugador
            // aparece en el punto de llegada que ese mundo define para este origen.
            let (body_min, body_max) = player.body();
            if let Some(destination) = scene.portal_touching(&body_min, &body_max) {
                let origin = realm;
                realm = destination;
                scene = build_realm(realm, current_biome);
                match scene.arrival_from(origin) {
                    Some(arrival) => player.arrive(arrival.eye, arrival.yaw, &scene),
                    None => player.unstick(&scene),
                }
                println!("portal: {origin:?} -> {realm:?}");
            }
        } else {
            let orbit = [
                (Key::Left, ROTATION_SPEED, 0.0),
                (Key::Right, -ROTATION_SPEED, 0.0),
                (Key::Up, 0.0, -ROTATION_SPEED),
                (Key::Down, 0.0, ROTATION_SPEED),
            ];

            for (key, delta_yaw, delta_pitch) in orbit {
                if window.is_key_down(key) {
                    camera.orbit(delta_yaw, delta_pitch);
                }
            }

            if window.is_key_down(Key::Equal) {
                camera.zoom(ZOOM_SPEED);
            }
            if window.is_key_down(Key::Minus) {
                camera.zoom(1.0 / ZOOM_SPEED);
            }
        }

        if window.is_key_pressed(Key::T, KeyRepeat::No) {
            auto_play = !auto_play;
        }

        let mut scene_changed = false;
        let biome_keys = [
            (Key::Key1, diorama::Biome::Overworld),
            (Key::Key2, diorama::Biome::Marine),
            (Key::Key3, diorama::Biome::Snow),
            (Key::Key4, diorama::Biome::Desert),
            (Key::Key5, diorama::Biome::Mesa),
            (Key::Key6, diorama::Biome::Sakura),
        ];
        for (key, biome) in biome_keys {
            if window.is_key_pressed(key, KeyRepeat::No) && (realm != Realm::Overworld || biome != current_biome) {
                current_biome = biome;
                realm = Realm::Overworld;
                scene = diorama::build_diorama(current_biome);
                scene_changed = true;
                println!("bioma: {}", current_biome.name());
            }
        }
        if window.is_key_pressed(Key::Key7, KeyRepeat::No) {
            realm = Realm::Nether;
            scene = nether::build_nether();
            scene_changed = true;
            println!("bioma: Nether");
        }
        if window.is_key_pressed(Key::Key8, KeyRepeat::No) {
            realm = Realm::End;
            scene = end::build_end();
            scene_changed = true;
            println!("bioma: End");
        }
        // Otra escena tiene otra geometria: si el jugador quedo dentro de un bloque, sale.
        if scene_changed {
            if let Some(player) = &mut player {
                player.unstick(&scene);
            }
        }

        if window.is_key_pressed(Key::V, KeyRepeat::No) {
            time_speed = (time_speed + 1) % TIME_SPEEDS.len();
            println!("velocidad del tiempo: {}", TIME_SPEEDS[time_speed].1);
        }

        if auto_play {
            time_of_day += AUTO_TIME_STEP * TIME_SPEEDS[time_speed].0;
        }
        if window.is_key_down(Key::Comma) {
            time_of_day -= MANUAL_TIME_STEP;
        }
        if window.is_key_down(Key::Period) {
            time_of_day += MANUAL_TIME_STEP;
        }
        time_of_day = time_of_day.rem_euclid(1.0);

        // El Nether y el End no tienen ciclo de dia: su iluminacion ya queda fija al construirlos.
        if realm == Realm::Overworld {
            let (mut sun, mut ambient, mut sky) = daycycle::lighting_at(time_of_day);
            apply_biome_mood(&mut sun, &mut ambient, &mut sky, current_biome);
            scene.lights[0] = sun;
            scene.ambient = ambient;
            scene.sky = sky;
        }

        // Se redibuja siempre (camara, ciclo del dia y agua en movimiento lo requieren);
        // el presupuesto de tiempo por frame sobra de sobra con la grilla de aceleracion.
        let free_view = player.as_ref().map(Player::camera);
        let (view, fov) = match &free_view {
            Some(free_camera) => (free_camera, FREE_FOV),
            None => (&camera, FOV),
        };
        render(&mut framebuffer, &scene, view, fov, clock.elapsed().as_secs_f32());

        window
            .update_with_buffer(&framebuffer.buffer, WIDTH, HEIGHT)
            .unwrap();

        // Solo se espera lo que falte para los 16 ms; si el cuadro ya tardo mas, no se duerme.
        std::thread::sleep(frame_delay.saturating_sub(last_frame.elapsed()));
    }
}
