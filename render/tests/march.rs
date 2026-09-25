//! The volume march, judged on a real frame of the film.

use render::gpu::{Gpu, Quality, Tracer};
use render::scene;

/// How alike the fine detail is in neighbouring pixel columns across a
/// grazing patch of gas. Step ripples are long contour lines, so they repeat
/// from one column to the next; sampling grain does not.
fn ripple_score() -> f32 {
    let (w, h) = (2048u32, 1080u32);
    let q = Quality {
        width: w,
        height: h,
        spp: 16,
        max_steps: 900,
        sky: 0.0,
        aperture: 0.0,
        wave_lens: 0.0,
        wave_ripple: 0.0,
    };
    let gpu = Gpu::new().unwrap();
    let mut tracer = Tracer::new(&gpu);
    let t = 50.0;
    let frames: Vec<_> = scene::shutter_times(t, q.spp)
        .iter()
        .map(|&s| scene::frame(s))
        .collect();
    let accum = tracer.render(&gpu, &frames, &q).unwrap();
    let lum = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        let p = &accum[i..i + 4];
        (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) / p[3]
    };
    // The remnant disk's near band, seen at a grazing angle, in half-4K pixels.
    let (x0, y0, n) = (350u32, 625u32, 160usize);
    let resid = |x: u32| -> Vec<f32> {
        let col: Vec<f32> = (0..n as u32).map(|k| lum(x, y0 + k)).collect();
        (0..n)
            .map(|k| {
                let (a, b) = (k.saturating_sub(3), (k + 4).min(n));
                col[k] - col[a..b].iter().sum::<f32>() / (b - a) as f32
            })
            .collect()
    };
    // Correlation of fine detail between neighbouring columns, best over a
    // small vertical shift since the ripples run on a shallow diagonal.
    let cols = 96;
    let mut total = 0.0;
    for c in 0..cols {
        let (a, b) = (resid(x0 + c * 3), resid(x0 + c * 3 + 1));
        let mut best = f32::MIN;
        for shift in -2i32..=2 {
            let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
            for k in 3..n - 3 {
                let (u, v) = (a[k], b[(k as i32 + shift) as usize]);
                ab += u * v;
                aa += u * u;
                bb += v * v;
            }
            best = best.max(ab / (aa * bb).sqrt().max(1e-12));
        }
        total += best;
    }
    total / cols as f32
}

#[test]
fn grazing_gas_has_no_step_ripples() {
    // Volume samples at fixed points in each step lined up across pixels
    // into contour ripples; at random points they average out to grain.
    // Measured 0.61 with midpoint samples, 0.22 with jittered ones.
    let score = ripple_score();
    assert!(score < 0.4, "fine detail repeats across columns: {score}");
}
