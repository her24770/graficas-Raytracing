use nalgebra_glm::Vec3;

use crate::camera::Camera;
use crate::scene::Scene;

// Cuerpo del jugador alrededor del ojo: 0.6 de ancho y 1.7 de alto, para que
// pase por una puerta de 1x2 bloques como en Minecraft.
const HALF_WIDTH: f32 = 0.3;
const EYE_TO_FEET: f32 = 1.5;
const EYE_TO_HEAD: f32 = 0.2;

const FLY_SPEED: f32 = 10.0;
const WALK_SPEED: f32 = 5.0;
const LOOK_SPEED: f32 = 1.8;
const PITCH_LIMIT: f32 = 1.5;

const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 8.0;
const MAX_FALL_SPEED: f32 = 30.0;
const VOID_Y: f32 = -45.0;

// Avance maximo por sub-paso: menor que el cuerpo y que un bloque, asi nunca se
// salta una pared entre un cuadro y el siguiente aunque el cuadro tarde mucho.
const MAX_STEP: f32 = 0.2;

#[derive(Default, Clone, Copy)]
pub struct Controls {
    pub forward: f32,
    pub strafe: f32,
    pub vertical: f32,
    pub turn: f32,
    pub look_up: f32,
    pub jump: bool,
}

pub struct Player {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
    walking: bool,
    vertical_speed: f32,
    on_ground: bool,
    spawn: Vec3,
}

impl Player {
    fn new(eye: Vec3, yaw: f32, pitch: f32) -> Self {
        Player {
            eye,
            yaw,
            pitch: pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT),
            walking: false,
            vertical_speed: 0.0,
            on_ground: false,
            spawn: eye,
        }
    }

    // Arranca donde esta la camara orbital y mirando hacia el mismo lado.
    pub fn from_camera(camera: &Camera, scene: &Scene) -> Self {
        let forward = (camera.center - camera.eye).normalize();
        let mut player = Player::new(camera.eye, forward.z.atan2(forward.x), forward.y.asin());
        player.unstick(scene);
        player.spawn = player.eye;
        player
    }

    pub fn is_walking(&self) -> bool {
        self.walking
    }

    pub fn toggle_walking(&mut self) {
        self.walking = !self.walking;
        self.vertical_speed = 0.0;
        self.on_ground = false;
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.sin(),
        )
    }

    pub fn camera(&self) -> Camera {
        Camera::new(self.eye, self.eye + self.forward(), Vec3::new(0.0, 1.0, 0.0))
    }

    fn collides(&self, scene: &Scene) -> bool {
        let (min, max) = self.body();
        scene.blocks_box(&min, &max)
    }

    pub fn body(&self) -> (Vec3, Vec3) {
        (
            self.eye - Vec3::new(HALF_WIDTH, EYE_TO_FEET, HALF_WIDTH),
            self.eye + Vec3::new(HALF_WIDTH, EYE_TO_HEAD, HALF_WIDTH),
        )
    }

    // Llegada por un portal: aparece en el punto indicado, mirando hacia `yaw`.
    pub fn arrive(&mut self, eye: Vec3, yaw: f32, scene: &Scene) {
        self.eye = eye;
        self.yaw = yaw;
        self.pitch = 0.0;
        self.vertical_speed = 0.0;
        self.on_ground = false;
        self.unstick(scene);
        self.spawn = self.eye;
    }

    // Si al cambiar de escena el jugador queda dentro de un bloque, sube hasta salir.
    pub fn unstick(&mut self, scene: &Scene) {
        for _ in 0..400 {
            if !self.collides(scene) {
                return;
            }
            self.eye.y += 0.25;
        }
    }

    // Mueve en un solo eje; devuelve true si choco. Al chocar se queda pegado a la
    // pared (biseccion) en vez de rebotar o quedar a medio camino.
    fn move_axis(&mut self, scene: &Scene, axis: usize, amount: f32) -> bool {
        let steps = (amount.abs() / MAX_STEP).ceil().max(1.0) as usize;
        let step = amount / steps as f32;

        for _ in 0..steps {
            let before = self.eye[axis];
            self.eye[axis] = before + step;

            if self.collides(scene) {
                let (mut free, mut blocked) = (0.0, step);
                for _ in 0..6 {
                    let middle = (free + blocked) * 0.5;
                    self.eye[axis] = before + middle;
                    if self.collides(scene) {
                        blocked = middle;
                    } else {
                        free = middle;
                    }
                }
                self.eye[axis] = before + free;
                return true;
            }
        }

        false
    }

    pub fn update(&mut self, scene: &Scene, controls: Controls, dt: f32) {
        self.yaw += controls.turn * LOOK_SPEED * dt;
        self.pitch =
            (self.pitch + controls.look_up * LOOK_SPEED * dt).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        // El avance es horizontal aunque se mire hacia arriba o abajo, como en Minecraft.
        let ahead = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let side = Vec3::new(-self.yaw.sin(), 0.0, self.yaw.cos());
        let mut direction = ahead * controls.forward + side * controls.strafe;
        if direction.magnitude() > 1.0 {
            direction = direction.normalize();
        }
        let speed = if self.walking { WALK_SPEED } else { FLY_SPEED };

        // Cada eje por separado: si una pared frena un eje, el otro sigue y se desliza.
        self.move_axis(scene, 0, direction.x * speed * dt);
        self.move_axis(scene, 2, direction.z * speed * dt);

        if !self.walking {
            self.move_axis(scene, 1, controls.vertical * FLY_SPEED * dt);
            return;
        }

        if controls.jump && self.on_ground {
            self.vertical_speed = JUMP_SPEED;
        }
        self.vertical_speed = (self.vertical_speed - GRAVITY * dt).max(-MAX_FALL_SPEED);

        let falling = self.vertical_speed < 0.0;
        let blocked = self.move_axis(scene, 1, self.vertical_speed * dt);
        self.on_ground = blocked && falling;
        if blocked {
            self.vertical_speed = 0.0;
        }

        // Si se cae de la isla, vuelve al punto de entrada y en modo vuelo.
        if self.eye.y < VOID_Y {
            self.eye = self.spawn;
            self.toggle_walking();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diorama::{build_diorama, Biome};
    use crate::{end, nether};

    const DT: f32 = 0.05;

    fn looking_at_origin_from(eye: Vec3) -> Player {
        let forward = (-eye).normalize();
        Player::new(eye, forward.z.atan2(forward.x), forward.y.asin())
    }

    #[test]
    fn volando_hacia_abajo_no_atraviesa_ninguna_escena() {
        for scene in [build_diorama(Biome::Overworld), nether::build_nether(), end::build_end()] {
            let mut player = Player::new(Vec3::new(0.3, 30.0, 0.3), 0.0, 0.0);
            let down = Controls { vertical: -1.0, ..Controls::default() };
            for _ in 0..200 {
                player.update(&scene, down, DT);
            }
            assert!(player.eye.y > -3.0, "atraveso el suelo: y = {}", player.eye.y);
            assert!(!player.collides(&scene));
        }
    }

    #[test]
    fn caminando_cae_y_se_apoya_en_el_suelo() {
        let scene = build_diorama(Biome::Overworld);
        let mut player = Player::new(Vec3::new(-6.0, 20.0, 6.0), 0.0, 0.0);
        player.toggle_walking();
        for _ in 0..200 {
            player.update(&scene, Controls::default(), DT);
        }
        assert!(player.on_ground, "no quedo apoyado, y = {}", player.eye.y);
        assert!(player.eye.y > -3.0 && player.eye.y < 12.0);

        let resting = player.eye.y;
        player.update(&scene, Controls { jump: true, ..Controls::default() }, DT);
        assert!(player.eye.y > resting, "el salto no lo levanto");
    }

    #[test]
    fn volando_de_frente_no_cruza_la_isla() {
        let scene = build_diorama(Biome::Overworld);
        // Sin colision terminaria 100 unidades mas alla, del otro lado de la isla.
        let mut player = Player::new(Vec3::new(40.0, -2.0, 0.0), std::f32::consts::PI, 0.0);
        let ahead = Controls { forward: 1.0, ..Controls::default() };
        for _ in 0..200 {
            player.update(&scene, ahead, DT);
        }
        assert!(player.eye.x > 0.0, "cruzo la isla: x = {}", player.eye.x);
    }

    use crate::scene::Realm;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn build(realm: Realm) -> Scene {
        match realm {
            Realm::Overworld => build_diorama(Biome::Overworld),
            Realm::Nether => nether::build_nether(),
            Realm::End => end::build_end(),
        }
    }

    fn arriving(scene: &Scene, from: Realm) -> Player {
        let arrival = scene.arrival_from(from).expect("falta el punto de llegada");
        let mut player = Player::new(Vec3::zeros(), 0.0, 0.0);
        player.arrive(arrival.eye, arrival.yaw, scene);
        player
    }

    // Avanza con los controles dados hasta tocar un portal o agotar los pasos.
    fn travel(scene: &Scene, player: &mut Player, controls: Controls, steps: usize) -> Option<Realm> {
        for _ in 0..steps {
            player.update(scene, controls, DT);
            let (min, max) = player.body();
            if let Some(destination) = scene.portal_touching(&min, &max) {
                return Some(destination);
            }
        }
        None
    }

    #[test]
    fn cada_llegada_queda_libre_y_fuera_de_los_portales() {
        let routes = [
            (Realm::Overworld, Realm::Nether),
            (Realm::Overworld, Realm::End),
            (Realm::Nether, Realm::Overworld),
            (Realm::End, Realm::Overworld),
        ];
        for (from, to) in routes {
            let origin = build(from);
            assert!(
                origin.portals.iter().any(|portal| portal.destination == to),
                "{from:?} no tiene portal hacia {to:?}"
            );

            let destination = build(to);
            let arrival = destination.arrival_from(from).expect("falta el punto de llegada");
            let player = arriving(&destination, from);
            assert_eq!(player.eye, arrival.eye, "llegada {from:?} -> {to:?} dentro de un bloque");
            let (min, max) = player.body();
            assert_eq!(
                destination.portal_touching(&min, &max),
                None,
                "llegada {from:?} -> {to:?} cae dentro de un portal"
            );
        }
    }

    #[test]
    fn el_tunel_oculto_lleva_al_portal_del_end() {
        let scene = build(Realm::Overworld);
        // Afuera de la isla, frente a la boca del tunel en la pared oeste.
        let mut player = Player::new(Vec3::new(-12.0 - 11.5, -2.0, 6.0 - 11.5), 0.0, 0.0);
        let ahead = Controls { forward: 1.0, ..Controls::default() };
        assert_eq!(travel(&scene, &mut player, ahead, 400), None, "el marco deberia frenar el paso");
        assert!(player.eye.x > -11.5, "no pudo entrar por el tunel: x = {}", player.eye.x);

        // Ya dentro de la sala: pasar por encima del marco alcanza para tocar el portal.
        player.eye.y += 0.9;
        assert_eq!(travel(&scene, &mut player, ahead, 20), Some(Realm::End));
    }

    #[test]
    fn la_sala_del_end_no_tiene_entrada_por_arriba() {
        let scene = build(Realm::Overworld);
        let mut player = Player::new(Vec3::new(3.0 - 11.5, 25.0, 6.0 - 11.5), 0.0, 0.0);
        let down = Controls { vertical: -1.0, ..Controls::default() };
        assert_eq!(travel(&scene, &mut player, down, 400), None);
        assert!(player.eye.y > 5.0, "entro a la sala desde arriba: y = {}", player.eye.y);
    }

    #[test]
    fn ida_y_vuelta_caminando_por_cada_portal() {
        let ahead = Controls { forward: 1.0, jump: true, ..Controls::default() };

        // Overworld -> Nether: desde delante del portal de la cueva, media vuelta y adentro.
        let overworld = build(Realm::Overworld);
        let mut player = arriving(&overworld, Realm::Nether);
        player.yaw = -FRAC_PI_2;
        assert_eq!(travel(&overworld, &mut player, ahead, 200), Some(Realm::Nether));

        // Nether -> Overworld: el portal de regreso queda a espaldas del punto de llegada.
        let nether = build(Realm::Nether);
        let mut player = arriving(&nether, Realm::Overworld);
        player.toggle_walking();
        player.yaw = FRAC_PI_2;
        assert_eq!(travel(&nether, &mut player, ahead, 200), Some(Realm::Overworld));

        // Overworld -> End: desde la sala oculta, media vuelta, saltar el marco y caer al portal.
        let mut player = arriving(&overworld, Realm::End);
        player.toggle_walking();
        player.yaw = PI + PI;
        assert_eq!(travel(&overworld, &mut player, ahead, 200), Some(Realm::End));

        // End -> Overworld: el portal de salida queda justo delante.
        let end = build(Realm::End);
        let mut player = arriving(&end, Realm::Overworld);
        player.toggle_walking();
        assert_eq!(travel(&end, &mut player, ahead, 200), Some(Realm::Overworld));
    }

    #[test]
    fn movimiento_al_azar_nunca_termina_dentro_de_un_bloque() {
        let scene = build_diorama(Biome::Overworld);
        let mut player = looking_at_origin_from(Vec3::new(12.0, 9.0, 12.0));
        assert!(!player.collides(&scene));

        let mut seed: u32 = 12345;
        let mut random = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 8) % 2001) as f32 / 1000.0 - 1.0
        };

        for step in 0..4000 {
            if step % 500 == 250 {
                player.toggle_walking();
            }
            let controls = Controls {
                forward: random(),
                strafe: random(),
                vertical: random(),
                turn: random(),
                look_up: random(),
                jump: random() > 0.5,
            };
            // Cuadros lentos incluidos: hasta 0.1 s, el tope que usa el bucle principal.
            let dt = 0.02 + (random() + 1.0) * 0.04;
            player.update(&scene, controls, dt);
            assert!(!player.collides(&scene), "quedo dentro de un bloque en el paso {step}");
        }
    }
}
