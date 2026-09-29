mod camera;
mod color;
mod cube;
mod diorama;
mod framebuffer;
mod light;
mod ray_intersect;
mod scene;
mod texture;

use minifb::{Key, Window, WindowOptions};
use nalgebra_glm::{dot, normalize, Vec3};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::camera::Camera;
use crate::color::Color;
use crate::framebuffer::Framebuffer;
use crate::ray_intersect::Intersect;
use crate::scene::Scene;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const FOV: f32 = PI / 4.0;
const ROTATION_SPEED: f32 = PI / 60.0;

const SHADOW_BIAS: f32 = 1e-3;
const MAX_SHADOW_CROSSINGS: usize = 6;

const SKY_HORIZON_LOW: u32 = 0xF2B866;
const SKY_MIDDLE: u32 = 0xDE8A6A;
const SKY_HIGH: u32 = 0x6E5A8C;

fn sky_color(ray_direction: &Vec3) -> Color {
    let t = ((ray_direction.y + 1.0) / 1.2).clamp(0.0, 1.0);
    if t < 0.5 {
        Color::lerp(Color::from_hex(SKY_HORIZON_LOW), Color::from_hex(SKY_MIDDLE), t / 0.5)
    } else {
        Color::lerp(Color::from_hex(SKY_MIDDLE), Color::from_hex(SKY_HIGH), (t - 0.5) / 0.5)
    }
}

pub fn reflect(incident: &Vec3, normal: &Vec3) -> Vec3 {
    incident - normal * (2.0 * dot(incident, normal))
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

fn shade(intersect: &Intersect, ray_origin: &Vec3, scene: &Scene) -> Color {
    let material = intersect.material;
    let base = scene.textures[intersect.texture_id]
        .sample(intersect.u, intersect.v)
        .to_vec3();
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

fn cast_ray(ray_origin: &Vec3, ray_direction: &Vec3, scene: &Scene) -> Color {
    let Some(intersect) = scene.trace(ray_origin, ray_direction) else {
        return sky_color(ray_direction);
    };

    shade(&intersect, ray_origin, scene)
}

fn render_band(
    band: &mut [u32],
    y_offset: usize,
    width: usize,
    height: usize,
    scene: &Scene,
    camera: &Camera,
) {
    let width_f = width as f32;
    let height_f = height as f32;
    let aspect_ratio = width_f / height_f;
    let perspective_scale = (FOV / 2.0).tan();

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

            band[local_y * width + x] = cast_ray(&camera.eye, &ray_direction, scene).to_hex();
        }
    }
}

fn render(framebuffer: &mut Framebuffer, scene: &Scene, camera: &Camera) {
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
                render_band(band, y_offset, width, height, scene, camera);
            });
        }
    });
}

fn main() {
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let scene = diorama::build_diorama();

    let mut camera = Camera::new(
        Vec3::new(27.5, 21.0, 27.5),
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );

    // Modo captura: `cargo run --release -- --screenshot salida.bmp [giro_en_pasos]`
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--screenshot" {
        if let Some(steps) = args.get(3).and_then(|value| value.parse::<i32>().ok()) {
            camera.orbit(steps as f32 * ROTATION_SPEED, 0.0);
        }
        let start = Instant::now();
        render(&mut framebuffer, &scene, &camera);
        println!("{} cubos, render en {:?}", scene.cubes.len(), start.elapsed());
        framebuffer
            .save_bmp(&args[2])
            .expect("no se pudo guardar la captura");
        return;
    }

    let mut window = Window::new("Diorama Raytracing", WIDTH, HEIGHT, WindowOptions::default())
        .expect("no se pudo crear la ventana");

    let frame_delay = Duration::from_millis(16);
    let mut camera_moved = true;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let orbit = [
            (Key::Left, ROTATION_SPEED, 0.0),
            (Key::Right, -ROTATION_SPEED, 0.0),
            (Key::Up, 0.0, -ROTATION_SPEED),
            (Key::Down, 0.0, ROTATION_SPEED),
        ];

        for (key, delta_yaw, delta_pitch) in orbit {
            if window.is_key_down(key) {
                camera.orbit(delta_yaw, delta_pitch);
                camera_moved = true;
            }
        }

        if camera_moved {
            render(&mut framebuffer, &scene, &camera);
            camera_moved = false;
        }

        window
            .update_with_buffer(&framebuffer.buffer, WIDTH, HEIGHT)
            .unwrap();

        std::thread::sleep(frame_delay);
    }
}
