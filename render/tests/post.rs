//! The post pass, on synthetic frames where the right answer is known.

use render::gpu::{Gpu, Look, Post};
use render::image::Hdr;

const W: u32 = 1920;
const H: u32 = 1010;

/// A patch of gas as hot and as large as the merger flash's brightest (its
/// top percent of pixels runs above 1 in linear light), with a black hole's
/// shadow beside it: empty, zero light.
fn flash_frame() -> Hdr {
    let mut rgb = vec![0.0f32; (W * H * 3) as usize];
    for y in 470..540 {
        for x in 250..550 {
            let i = ((y * W + x) * 3) as usize;
            rgb[i..i + 3].copy_from_slice(&[5.5, 5.0, 4.2]);
        }
    }
    Hdr {
        width: W,
        height: H,
        rgb,
    }
}

fn shadow_value(look: &Look) -> f32 {
    let gpu = Gpu::new().unwrap();
    let post = Post::new(&gpu);
    let out = post
        .process(&gpu, &flash_frame(), look, 1.0, 1.15, 0)
        .unwrap();
    // Inside the shadow, level with the hot gas and a shadow's width from it,
    // where the flash frame's veil was worst.
    let (x, y) = (1000u32, 505u32);
    let i = ((y * W + x) * 4) as usize;
    out[i..i + 3].iter().sum::<f32>() / 3.0
}

#[test]
fn flash_leaves_the_shadow_dark() {
    // Glare may hug the hot gas; the shadow level with it stays black. The
    // anamorphic streak, fed by all light, smeared the flash across it.
    let v = shadow_value(&Look::default());
    assert!(v < 0.05, "shadow lifted to {v}");
}
