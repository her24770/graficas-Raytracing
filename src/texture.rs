use crate::color::Color;

pub struct Texture {
    pub width: usize,
    pub height: usize,
    pixels: Vec<Color>,
}

impl Texture {
    pub fn from_bmp(path: &str) -> Texture {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|err| panic!("no se pudo leer la textura {path}: {err}"));

        assert_eq!(&bytes[0..2], b"BM", "{path} no es un archivo BMP válido");

        let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
        let bits_per_pixel = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
        let compression = u32::from_le_bytes(bytes[30..34].try_into().unwrap());

        assert_eq!(
            bits_per_pixel, 24,
            "{path}: solo se soportan BMP de 24 bits sin paleta"
        );
        assert_eq!(compression, 0, "{path}: solo se soportan BMP sin compresión");

        let flip_vertical = height > 0;
        let width = width.unsigned_abs() as usize;
        let height = height.unsigned_abs() as usize;

        let row_size = (width * 3).div_ceil(4) * 4;
        let mut pixels = vec![Color::new(0, 0, 0); width * height];

        for row in 0..height {
            let src_row = if flip_vertical { height - 1 - row } else { row };
            let row_start = pixel_offset + src_row * row_size;

            for col in 0..width {
                let i = row_start + col * 3;
                let (b, g, r) = (bytes[i], bytes[i + 1], bytes[i + 2]);
                pixels[row * width + col] = Color::new(r, g, b);
            }
        }

        Texture {
            width,
            height,
            pixels,
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> Color {
        let u = u.rem_euclid(1.0);
        let v = v.rem_euclid(1.0);

        let x = ((u * self.width as f32) as usize).min(self.width - 1);
        let y = ((v * self.height as f32) as usize).min(self.height - 1);

        self.pixels[y * self.width + x]
    }
}
