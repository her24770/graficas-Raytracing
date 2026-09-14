mod camera;
mod color;
mod cube;
mod framebuffer;
mod ray_intersect;
mod scene;
mod texture;

use minifb::{Key, Window, WindowOptions};
use nalgebra_glm::{normalize, Vec3};
use std::f32::consts::PI;
use std::time::Duration;

use crate::camera::Camera;
use crate::color::Color;
use crate::cube::Cube;
use crate::framebuffer::Framebuffer;
use crate::ray_intersect::{Material, RayIntersect};
use crate::scene::Scene;
use crate::texture::Texture;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const BACKGROUND_COLOR: u32 = 0x040C24;

const FOV: f32 = PI / 3.0;
const ROTATION_SPEED: f32 = PI / 60.0;

fn cast_ray(ray_origin: &Vec3, ray_direction: &Vec3, scene: &Scene) -> Color {
    let mut closest = None;

    for cube in &scene.cubes {
        if let Some(intersect) = cube.ray_intersect(ray_origin, ray_direction) {
            if closest.is_none_or(|current: crate::ray_intersect::Intersect| {
                intersect.distance < current.distance
            }) {
                closest = Some(intersect);
            }
        }
    }

    let Some(intersect) = closest else {
        return scene.background_color;
    };

    let texture = &scene.textures[intersect.material.texture_id];
    texture.sample(intersect.u, intersect.v)
}

fn render(framebuffer: &mut Framebuffer, scene: &Scene, camera: &Camera) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let aspect_ratio = width / height;

    let perspective_scale = (FOV / 2.0).tan();

    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let screen_x = (2.0 * x as f32) / width - 1.0;
            let screen_y = -(2.0 * y as f32) / height + 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = normalize(&Vec3::new(screen_x, screen_y, -1.0));
            let ray_direction = camera.basis_change(&ray_direction);

            framebuffer.set_current_color(cast_ray(&camera.eye, &ray_direction, scene).to_hex());
            framebuffer.point(x, y);
        }
    }
}

fn build_test_scene() -> Scene {
    let textures = vec![Texture::from_bmp("assets/textures/debug_uv.bmp")];
    let debug = Material::new(0);

    let mut cubes = Vec::new();

    for grid_x in -1..=1 {
        for grid_z in -1..=1 {
            cubes.push(Cube::new(
                Vec3::new(grid_x as f32 * 1.1, -1.0, grid_z as f32 * 1.1),
                Vec3::new(0.98, 0.98, 0.98),
                debug,
            ));
        }
    }

    cubes.push(Cube::new(Vec3::new(0.0, 0.1, 0.0), Vec3::new(1.0, 1.0, 1.0), debug));
    cubes.push(Cube::new(Vec3::new(2.0, 0.6, -0.6), Vec3::new(0.6, 1.6, 0.6), debug));

    Scene::new(cubes, textures, Color::from_hex(BACKGROUND_COLOR))
}

fn main() {
    let frame_delay = Duration::from_millis(16);

    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut window = Window::new("Diorama Raytracing", WIDTH, HEIGHT, WindowOptions::default())
        .expect("no se pudo crear la ventana");

    let scene = build_test_scene();

    let mut camera = Camera::new(
        Vec3::new(0.0, 2.0, 6.0),
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );

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
