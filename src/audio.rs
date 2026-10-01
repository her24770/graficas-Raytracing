use std::process::{Child, Command, Stdio};

// Musica de fondo sin librerias de audio: se lanza `afplay`, el reproductor que
// trae macOS, como proceso aparte. En otros sistemas no existe y el programa
// simplemente corre en silencio.
pub struct Music {
    path: &'static str,
    player: Option<Child>,
    enabled: bool,
}

impl Music {
    pub fn start(path: &'static str) -> Self {
        let mut music = Music { path, player: None, enabled: true };
        music.play();
        music
    }

    fn play(&mut self) {
        self.player = Command::new("afplay")
            .arg(self.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        // Sin reproductor no se reintenta en cada cuadro.
        self.enabled = self.player.is_some();
    }

    fn stop(&mut self) {
        if let Some(mut player) = self.player.take() {
            let _ = player.kill();
            let _ = player.wait();
        }
    }

    pub fn toggle(&mut self) {
        if self.player.is_some() {
            self.stop();
            self.enabled = false;
        } else {
            self.play();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.player.is_some()
    }

    // Se llama una vez por cuadro: cuando la pista termina, vuelve a empezar.
    pub fn keep_looping(&mut self) {
        if !self.enabled {
            return;
        }
        let Some(player) = &mut self.player else { return };
        match player.try_wait() {
            Ok(Some(status)) if status.success() => self.play(),
            // Termino con error (por ejemplo, falta el archivo): no insistir.
            Ok(Some(_)) | Err(_) => {
                self.player = None;
                self.enabled = false;
            }
            Ok(None) => {}
        }
    }
}

impl Drop for Music {
    fn drop(&mut self) {
        self.stop();
    }
}
