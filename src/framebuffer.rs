pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Framebuffer {
            width,
            height,
            buffer: vec![0; width * height],
        }
    }

    pub fn save_bmp(&self, path: &str) -> std::io::Result<()> {
        let row_size = (self.width * 3).div_ceil(4) * 4;
        let pixel_bytes = row_size * self.height;
        let padding = row_size - self.width * 3;

        let mut data = Vec::with_capacity(54 + pixel_bytes);
        data.extend_from_slice(b"BM");
        data.extend_from_slice(&((54 + pixel_bytes) as u32).to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&54u32.to_le_bytes());
        data.extend_from_slice(&40u32.to_le_bytes());
        data.extend_from_slice(&(self.width as i32).to_le_bytes());
        data.extend_from_slice(&(self.height as i32).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&24u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        for y in (0..self.height).rev() {
            for x in 0..self.width {
                let pixel = self.buffer[y * self.width + x];
                data.push((pixel & 0xFF) as u8);
                data.push(((pixel >> 8) & 0xFF) as u8);
                data.push(((pixel >> 16) & 0xFF) as u8);
            }
            data.extend(std::iter::repeat_n(0u8, padding));
        }

        std::fs::write(path, data)
    }
}
