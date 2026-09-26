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

/// Display value of a hot dot's glow, a little way off it at a fixed fraction
/// of the frame, clear of the streak's horizontal axis.
fn glow_at_scale(w: u32, h: u32) -> f32 {
    let mut rgb = vec![0.0f32; (w * h * 3) as usize];
    let (cx, cy, r) = (w / 2, h / 2, w / 160);
    for y in cy - r..cy + r {
        for x in cx - r..cx + r {
            let i = ((y * w + x) * 3) as usize;
            rgb[i..i + 3].copy_from_slice(&[4.0, 3.6, 3.0]);
        }
    }
    let gpu = Gpu::new().unwrap();
    let post = Post::new(&gpu);
    let look = Look {
        grain: 0.0,
        ..Look::default()
    };
    let out = post
        .process(
            &gpu,
            &Hdr {
                width: w,
                height: h,
                rgb,
            },
            &look,
            0.0,
            1.0,
            0,
        )
        .unwrap();
    let (x, y) = (cx + w / 64, cy + h / 24);
    let i = ((y * w + x) * 4) as usize;
    out[i..i + 3].iter().sum::<f32>() / 3.0
}

#[test]
fn glow_is_the_same_size_at_any_resolution() {
    // Looks are judged on previews and shipped at 4K. Blur radii fixed in
    // pixels made the 4K glow half as wide as the preview's.
    let small = glow_at_scale(1920, 1010);
    let large = glow_at_scale(3840, 2020);
    assert!(small > 0.01, "no glow to compare: {small}");
    // The pyramid's sampling leaves about 13%; fixed radii missed by 34%.
    assert!((small - large).abs() < 0.2 * small, "{small} vs {large}");
}
