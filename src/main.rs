mod camera;
mod color;
mod cube;
mod diorama;
mod framebuffer;
mod ray_intersect;
mod scene;
mod texture;

use minifb::{Key, Window, WindowOptions};
use nalgebra_glm::{normalize, Vec3};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::camera::Camera;
use crate::color::Color;
use crate::framebuffer::Framebuffer;
use crate::scene::Scene;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const FOV: f32 = PI / 4.0;
const ROTATION_SPEED: f32 = PI / 60.0;

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

// Sombreado fijo por orientacion de cara; se reemplaza por Phong en la etapa de iluminacion.
fn face_shade(normal: &Vec3) -> f32 {
    if normal.y > 0.5 {
        1.0
    } else if normal.y < -0.5 {
        0.5
    } else if normal.z.abs() > 0.5 {
        0.82
    } else {
        0.66
    }
}

fn cast_ray(ray_origin: &Vec3, ray_direction: &Vec3, scene: &Scene) -> Color {
    let Some(intersect) = scene.trace(ray_origin, ray_direction) else {
        return sky_color(ray_direction);
    };

    let texture = &scene.textures[intersect.texture_id];
    texture.sample(intersect.u, intersect.v) * face_shade(&intersect.normal)
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
