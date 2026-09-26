//! The trace shader, probed one ray at a time: a camera with a hair-thin field
//! of view sees a single geodesic, so a pixel reads what that ray collected.

use render::gpu::{Gpu, Quality, Tracer};
use render::scene::{self, Camera, Frame};

const SIZE: u32 = 8;

fn probe_quality() -> Quality {
    Quality {
        width: SIZE,
        height: SIZE,
        spp: 1,
        max_steps: 4000,
        sky: 0.0,
        aperture: 0.0,
        wave_lens: 0.0,
        wave_ripple: 0.0,
    }
}

fn aim(frame: &mut Frame, pos: [f32; 3], look_at: [f32; 3], up: [f32; 3]) {
    frame.camera = Camera {
        pos,
        look_at,
        up,
        roll_deg: 0.0,
        fov_x_deg: 0.01,
        aperture: 0.0,
        focus_distance: 1.0,
    };
}

/// Mean RGB of the probe image.
fn trace(tracer: &mut Tracer, gpu: &Gpu, frame: &Frame) -> [f32; 3] {
    let accum = tracer.render(gpu, frame, &probe_quality()).unwrap();
    let mut sum = [0.0; 3];
    for px in accum.as_chunks::<4>().0 {
        for c in 0..3 {
            sum[c] += px[c] / px[3];
        }
    }
    sum.map(|s| s / (SIZE * SIZE) as f32)
}

fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

#[test]
fn corona_glows_above_the_gas() {
    // A ray skimming the big disk above its gas slab crosses only corona. The
    // march once added emission only where there was gas, so this ray came
    // back black and the corona ended in a hard edge at the slab.
    let gpu = Gpu::new().unwrap();
    let mut tracer = Tracer::new(&gpu);
    let mut frame = scene::frame(20.0);
    let outer = frame.circumbinary_outer * 1.15;
    let slab = 3.0 * (0.009 * outer + 0.025);
    let y = slab + 0.35;
    aim(
        &mut frame,
        [-80.0, y, 22.0],
        [0.0, y, 22.0],
        [0.0, 1.0, 0.0],
    );
    // Lensing dips the ray into the far edge of the gas, so the old march
    // still collected about 6e-5. The corona itself brings about 6e-3.
    let lum = luminance(trace(&mut tracer, &gpu, &frame));
    assert!(lum > 1e-3, "corona above the slab is dark: {lum}");
}

#[test]
fn streams_hand_over_at_the_mini_disk_rim() {
    // Looking straight down onto a stream inside a mini-disk's rim, the stream
    // adds nothing: it feeds the rim. Carried on to the hole, it met the
    // horizon at full strength and rays that ended there cut it into boxes.
    let gpu = Gpu::new().unwrap();
    let mut tracer = Tracer::new(&gpu);
    let mut with_streams = scene::frame(40.0);
    assert!(!with_streams.merged);
    let hole = with_streams.bodies[0].pos;
    let rim = with_streams.bodies[0].disk_outer;
    let rho = (hole[0] * hole[0] + hole[2] * hole[2]).sqrt();
    let reach = 0.55 * rim;
    let target = [
        hole[0] * (1.0 + reach / rho),
        0.0,
        hole[2] * (1.0 + reach / rho),
    ];
    aim(
        &mut with_streams,
        [target[0], 30.0, target[2]],
        target,
        [1.0, 0.0, 0.0],
    );
    // Streams are drawn only before the merger; the flag is their switch.
    let mut without = with_streams;
    without.merged = true;
    let a = trace(&mut tracer, &gpu, &with_streams);
    let b = trace(&mut tracer, &gpu, &without);
    let diff = luminance([a[0] - b[0], a[1] - b[1], a[2] - b[2]]).abs();
    assert!(
        diff <= 1e-6 + 1e-4 * luminance(b),
        "stream inside the rim adds {diff} over {}",
        luminance(b)
    );
}
