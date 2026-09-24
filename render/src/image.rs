use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Linear RGB, row-major, 3 floats per pixel.
pub struct Hdr {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<f32>,
}

impl Hdr {
    pub fn from_rgba(width: u32, height: u32, rgba: &[f32]) -> Self {
        let rgb = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let n = if p[3] > 0.0 { 1.0 / p[3] } else { 0.0 };
                [p[0] * n, p[1] * n, p[2] * n]
            })
            .collect();
        Hdr { width, height, rgb }
    }

    /// Half-float RGB with ZIP compression: about a fifth of the raw size on
    /// these mostly dark frames, and more than enough precision to regrade.
    pub fn write_exr(&self, path: &Path) -> Result<(), String> {
        use exr::prelude::*;
        let w = self.width as usize;
        let channels = SpecificChannels::rgb(|p: Vec2<usize>| {
            let i = (p.y() * w + p.x()) * 3;
            (
                f16::from_f32(self.rgb[i]),
                f16::from_f32(self.rgb[i + 1]),
                f16::from_f32(self.rgb[i + 2]),
            )
        });
        let mut image = Image::from_channels((w, self.height as usize), channels);
        image.layer_data.encoding = Encoding::SMALL_LOSSLESS;
        image
            .write()
            .to_file(path)
            .map_err(|e| format!("write {}: {e}", path.display()))
    }

    pub fn read_exr(path: &Path) -> Result<Self, String> {
        let image = exr::prelude::read_first_rgba_layer_from_file(
            path,
            |res, _| Hdr {
                width: res.width() as u32,
                height: res.height() as u32,
                rgb: vec![0.0; res.width() * res.height() * 3],
            },
            |img, pos, (r, g, b, _): (f32, f32, f32, f32)| {
                let i = (pos.y() * img.width as usize + pos.x()) * 3;
                img.rgb[i] = r;
                img.rgb[i + 1] = g;
                img.rgb[i + 2] = b;
            },
        )
        .map_err(|e| format!("read {}: {e}", path.display()))?;
        Ok(image.layer_data.channel_data.pixels)
    }
}

/// Write display-referred RGBA floats in [0, 1] as a 16-bit RGB PNG.
pub fn write_png16(path: &Path, width: u32, height: u32, rgba: &[f32]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Sixteen);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(rgba.len() / 4 * 6);
    for p in rgba.as_chunks::<4>().0 {
        for c in &p[..3] {
            let v = (c.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
            data.extend_from_slice(&v.to_be_bytes());
        }
    }
    writer.write_image_data(&data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exr_round_trips_within_half_precision() {
        let (w, h) = (64, 8);
        let rgb: Vec<f32> = (0..w * h * 3).map(|i| (i as f32 * 0.37) % 5.0).collect();
        let hdr = Hdr {
            width: w as u32,
            height: h as u32,
            rgb: rgb.clone(),
        };
        let dir = std::env::temp_dir().join("black-hole-exr-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rt.exr");
        hdr.write_exr(&path).unwrap();
        let back = Hdr::read_exr(&path).unwrap();
        assert_eq!((back.width, back.height), (hdr.width, hdr.height));
        for (a, b) in rgb.iter().zip(&back.rgb) {
            assert!((a - b).abs() <= a.abs() * 2e-3 + 1e-4, "{a} vs {b}");
        }
    }
}
