use nalgebra_glm::Vec3;

use crate::color::Color;
use crate::light::Light;
use crate::scene::{Ambient, SkyGradient};

struct Keyframe {
    time: f32,
    direction: (f32, f32, f32),
    sun_color: u32,
    sun_intensity: f32,
    sky: u32,
    ground: u32,
    ambient_intensity: f32,
    // Degradado del cielo de fondo: horizonte, medio y cenit.
    sky_gradient: (u32, u32, u32),
}

// Ciclo estilizado del dia, no astronomicamente exacto: noche fria y tenue,
// amanecer rosado, mediodia brillante y neutro, y la "hora dorada" que se ajusto a mano.
const KEYFRAMES: [Keyframe; 5] = [
    Keyframe {
        time: 0.0,
        direction: (0.2, 0.5, -0.3),
        sun_color: 0x4A5596, // Mas azul marino y oscuro
        sun_intensity: 0.12,  // Muy oscuro en la noche
        sky: 0x1A1A33,
        ground: 0x1A1E2D,
        ambient_intensity: 0.1, // Noche oscura
        sky_gradient: (0x0F0F1A, 0x14142B, 0x050510),
    },
    Keyframe {
        time: 0.22,
        direction: (0.7, 0.25, 0.4),
        sun_color: 0xFFD8B0, // Tono mas calido de amanecer
        sun_intensity: 1.0,
        sky: 0xC49BB8,
        ground: 0x8C7485,
        ambient_intensity: 0.45,
        sky_gradient: (0xFFC0A0, 0xE5A5B8, 0x60659A),
    },
    Keyframe {
        time: 0.5,
        direction: (0.2, 0.9, 0.3),
        sun_color: 0xFFF8E6, // Mediodia brillante
        sun_intensity: 1.5,
        sky: 0x8FB8E0,
        ground: 0x9C8F72,
        ambient_intensity: 0.65,
        sky_gradient: (0xCFE6F5, 0x9FCBEA, 0x4A86C8),
    },
    Keyframe {
        time: 0.78,
        direction: (-0.6, 0.32, 0.73),
        sun_color: 0xFF6B1A, // Naranja muy fuerte de atardecer
        sun_intensity: 1.6,
        sky: 0x9B62A6,
        ground: 0xB5653C,
        ambient_intensity: 0.5,
        sky_gradient: (0xFF7A00, 0xE85A3A, 0x624A8C), // Muy vibrante
    },
    Keyframe {
        time: 1.0,
        direction: (0.2, 0.5, -0.3),
        sun_color: 0x4A5596, // Lo mismo que time: 0.0
        sun_intensity: 0.12,
        sky: 0x1A1A33,
        ground: 0x1A1E2D,
        ambient_intensity: 0.1,
        sky_gradient: (0x0F0F1A, 0x14142B, 0x050510),
    },
];

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: u32, b: u32, t: f32) -> Color {
    Color::lerp(Color::from_hex(a), Color::from_hex(b), t)
}

pub fn lighting_at(time_of_day: f32) -> (Light, Ambient, SkyGradient) {
    let t = time_of_day.rem_euclid(1.0);
    let idx = KEYFRAMES
        .iter()
        .position(|frame| frame.time >= t)
        .unwrap_or(KEYFRAMES.len() - 1)
        .max(1);

    let a = &KEYFRAMES[idx - 1];
    let b = &KEYFRAMES[idx];
    let local_t = ((t - a.time) / (b.time - a.time)).clamp(0.0, 1.0);

    let direction = Vec3::new(
        lerp_f32(a.direction.0, b.direction.0, local_t),
        lerp_f32(a.direction.1, b.direction.1, local_t),
        lerp_f32(a.direction.2, b.direction.2, local_t),
    )
    .normalize();

    let sun_color = lerp_color(a.sun_color, b.sun_color, local_t);
    let sun_intensity = lerp_f32(a.sun_intensity, b.sun_intensity, local_t);

    let ambient = Ambient {
        sky: lerp_color(a.sky, b.sky, local_t),
        ground: lerp_color(a.ground, b.ground, local_t),
        intensity: lerp_f32(a.ambient_intensity, b.ambient_intensity, local_t),
    };

    let sun = Light::new(direction * 1000.0, sun_color, sun_intensity, f32::INFINITY);

    let sky_gradient = SkyGradient {
        horizon: lerp_color(a.sky_gradient.0, b.sky_gradient.0, local_t),
        middle: lerp_color(a.sky_gradient.1, b.sky_gradient.1, local_t),
        high: lerp_color(a.sky_gradient.2, b.sky_gradient.2, local_t),
    };

    (sun, ambient, sky_gradient)
}
