//! The loop timeline: where the black holes are, where the camera is, and how
//! bright the light wash is, all as pure functions of loop time.
//!
//! Units: G = c = 1 and the total mass is 1, so the merged Schwarzschild radius
//! is 2. One loop second is not one geometric time unit; orbital rates are
//! scaled for the eye, see [`orbital_rate`].

pub const FPS: f32 = 24.0;
pub const LOOP_SECONDS: f32 = 60.0;
pub const LOOP_FRAMES: u32 = 1440;

/// Masses of the two bodies. They sum to 1.
pub const M1: f32 = 0.58;
pub const M2: f32 = 0.42;
/// Mass radiated away as gravitational waves at merger.
pub const MERGER_LOSS: f32 = 0.05;

/// Separation at t = 0.
pub const D0: f32 = 12.0;
/// Coalescence time of the Peters inspiral, in loop seconds.
pub const T_COALESCE: f32 = 41.0;
/// After the wash peaks the scene snaps back to the opening so that frame 1440
/// equals frame 0.
pub const T_CUT: f32 = 58.5;
/// The wash starts rising here and is fully gone again at `T_WASH_END`, so
/// the pure-white band around the cut lasts well under a second.
pub const T_WASH_START: f32 = 56.0;
pub const T_WASH_END: f32 = 59.6;

pub const RINGDOWN_AMPLITUDE: f32 = 2.2;
pub const RINGDOWN_OMEGA: f32 = 5.2;
pub const RINGDOWN_TAU: f32 = 2.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub mass: f32,
    /// Schwarzschild radius, 2 * mass.
    pub rs: f32,
    pub disk_inner: f32,
    pub disk_outer: f32,
    /// Emission multiplier; 0 hides the disk.
    pub disk_gain: f32,
    pub disk_temp: f32,
    /// Disk normal.
    pub disk_normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub pos: [f32; 3],
    pub look_at: [f32; 3],
    pub up: [f32; 3],
    pub fov_x_deg: f32,
    pub aperture: f32,
    pub focus_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Loop time in seconds, 0 <= t < 60.
    pub t: f32,
    /// Physical scene time. Negative in the last 1.5 seconds, when the opening
    /// shot is already on screen under the wash.
    pub tau: f32,
    pub bodies: [Body; 2],
    pub circumbinary_inner: f32,
    pub circumbinary_outer: f32,
    pub circumbinary_gain: f32,
    pub camera: Camera,
    /// 0 normally, 1 at the loop seam.
    pub wash: f32,
    pub merged: bool,
}

pub fn frame_time(frame: u32) -> f32 {
    (frame % LOOP_FRAMES) as f32 / FPS
}

/// Loop time to physical time. The cut happens at `T_CUT`; after it the
/// opening is playing at negative physical time.
pub fn physical_time(t: f32) -> f32 {
    let t = t.rem_euclid(LOOP_SECONDS);
    if t < T_CUT { t } else { t - LOOP_SECONDS }
}

/// Separation of the two bodies. Peters inspiral before coalescence, a damped
/// quasi-normal wobble after it.
pub fn separation(tau: f32) -> f32 {
    if tau < T_COALESCE {
        D0 * (1.0 - tau / T_COALESCE).powf(0.25)
    } else {
        let s = tau - T_COALESCE;
        RINGDOWN_AMPLITUDE * (-s / RINGDOWN_TAU).exp() * (RINGDOWN_OMEGA * s).sin()
    }
}

/// Orbital angular rate in radians per loop second. Kepler's shape, scaled so
/// the opening orbit takes about twelve seconds and capped so the final orbits
/// stay readable.
pub fn orbital_rate(d: f32) -> f32 {
    const K: f32 = 22.0;
    const MAX: f32 = 7.0;
    (K * d.abs().max(0.5).powf(-1.5)).min(MAX)
}

/// Orbital phase, integrated numerically from t = -4 so the pre-opening frames
/// share the same curve.
pub fn orbital_phase(tau: f32) -> f32 {
    const T0: f32 = -4.0;
    const DT: f32 = 1.0 / 480.0;
    let end = tau.min(T_COALESCE);
    let mut phi = 0.0;
    let mut t = T0;
    while t < end {
        let h = DT.min(end - t);
        phi += orbital_rate(separation(t + 0.5 * h)) * h;
        t += h;
    }
    if tau > T_COALESCE {
        // Ringdown wobbles along the axis where the merger happened.
        phi += 0.6 * (tau - T_COALESCE).min(1.0);
    }
    phi
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn wash(t: f32) -> f32 {
    let t = t.rem_euclid(LOOP_SECONDS);
    if t < T_CUT {
        smoothstep(T_WASH_START, T_CUT, t)
    } else {
        1.0 - smoothstep(T_CUT, T_WASH_END, t)
    }
}

/// Catmull-Rom keyframes for the camera: (tau, distance, elevation deg, azimuth deg, fov deg).
const CAMERA_KEYS: [[f32; 5]; 9] = [
    [-6.0, 78.0, 18.0, -14.0, 36.0],
    [0.0, 74.0, 17.0, 0.0, 36.0],
    [20.0, 60.0, 14.0, 42.0, 36.0],
    [38.0, 46.0, 12.0, 84.0, 36.0],
    [42.0, 40.0, 18.0, 96.0, 38.0],
    [47.0, 36.0, 28.0, 116.0, 40.0],
    [52.0, 30.0, 36.0, 136.0, 40.0],
    [58.5, 15.0, 34.0, 158.0, 42.0],
    [61.0, 12.0, 36.0, 166.0, 42.0],
];

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, s: f32) -> f32 {
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * s
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * s * s
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * s * s * s)
}

fn camera_key(tau: f32) -> [f32; 4] {
    let n = CAMERA_KEYS.len();
    let tau = tau.clamp(CAMERA_KEYS[0][0], CAMERA_KEYS[n - 1][0]);
    let mut i = 0;
    while i + 2 < n && CAMERA_KEYS[i + 1][0] <= tau {
        i += 1;
    }
    let k1 = CAMERA_KEYS[i];
    let k2 = CAMERA_KEYS[i + 1];
    let k0 = CAMERA_KEYS[i.saturating_sub(1)];
    let k3 = CAMERA_KEYS[(i + 2).min(n - 1)];
    let s = ((tau - k1[0]) / (k2[0] - k1[0])).clamp(0.0, 1.0);
    let mut out = [0.0; 4];
    for (j, o) in out.iter_mut().enumerate() {
        *o = catmull_rom(k0[j + 1], k1[j + 1], k2[j + 1], k3[j + 1], s);
    }
    out
}

pub fn camera(tau: f32) -> Camera {
    let [dist, elev, azim, fov] = camera_key(tau);
    let (el, az) = (elev.to_radians(), azim.to_radians());
    let pos = [
        dist * el.cos() * az.sin(),
        dist * el.sin(),
        dist * el.cos() * az.cos(),
    ];
    Camera {
        pos,
        look_at: [0.0, 0.4, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_x_deg: fov,
        aperture: 0.0,
        focus_distance: dist,
    }
}

pub fn frame(t: f32) -> Frame {
    let tau = physical_time(t);
    let d = separation(tau);
    let phi = orbital_phase(tau);
    let dir = [phi.cos(), 0.0, phi.sin()];
    let tangent = [-phi.sin(), 0.0, phi.cos()];
    let merged = tau >= T_COALESCE;
    // Physical orbital speed for Doppler, not the cinematic angular rate.
    let v_orb = (1.0 / d.abs().max(1.0)).sqrt();

    let mut bodies = [Body {
        pos: [0.0; 3],
        vel: [0.0; 3],
        mass: 0.0,
        rs: 0.0,
        disk_inner: 0.0,
        disk_outer: 0.0,
        disk_gain: 0.0,
        disk_temp: 9000.0,
        disk_normal: [0.0, 1.0, 0.0],
    }; 2];

    let masses = if merged {
        let m = 1.0 - MERGER_LOSS;
        [m * M1, m * M2]
    } else {
        [M1, M2]
    };
    let offsets = [M2 * d, -M1 * d];
    let tilts = [0.12_f32, -0.16_f32];
    for i in 0..2 {
        let m = masses[i];
        let rs = 2.0 * m;
        let sign = if i == 0 { 1.0 } else { -1.0 };
        let speed = sign * v_orb * (1.0 - m);
        let disk_inner = 3.0 * rs;
        let tidal = 0.36 * d.abs();
        let disk_outer = tidal.max(disk_inner * 1.25);
        let strip = smoothstep(3.0, 6.0, d.abs());
        bodies[i] = Body {
            pos: [offsets[i] * dir[0], 0.0, offsets[i] * dir[2]],
            vel: [speed * tangent[0], 0.0, speed * tangent[2]],
            mass: m,
            rs,
            disk_inner,
            disk_outer,
            disk_gain: if merged { 0.0 } else { 0.7 * strip },
            disk_temp: 6800.0,
            disk_normal: [
                tilts[i].sin() * dir[2],
                tilts[i].cos(),
                -tilts[i].sin() * dir[0],
            ],
        };
    }
    if merged {
        // The ringdown is the two centres wobbling through each other; the
        // emission comes from a single disk forming around the remnant.
        let s = tau - T_COALESCE;
        let grow = smoothstep(0.0, 8.0, s);
        bodies[0].disk_inner = 3.0 * 2.0 * (1.0 - MERGER_LOSS);
        bodies[0].disk_outer = bodies[0].disk_inner * (1.3 + 1.7 * grow);
        bodies[0].disk_gain = 0.5 * smoothstep(0.0, 2.5, s);
        bodies[0].disk_temp = 6200.0;
        bodies[0].disk_normal = [0.0, 1.0, 0.0];
        bodies[0].vel = [0.0; 3];
        bodies[1].vel = [0.0; 3];
    }

    let circumbinary_inner = if merged {
        let s = tau - T_COALESCE;
        13.0 - 4.0 * smoothstep(0.0, 12.0, s)
    } else {
        (1.35 * d).max(13.0)
    };

    Frame {
        t: t.rem_euclid(LOOP_SECONDS),
        tau,
        bodies,
        circumbinary_inner,
        circumbinary_outer: 28.0,
        circumbinary_gain: 1.2,
        camera: camera(tau),
        wash: wash(t),
        merged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    #[test]
    fn loop_seam_is_exact() {
        let a = frame(0.0);
        let b = frame(LOOP_SECONDS);
        assert_eq!(a, b);
        assert_eq!(frame_time(0), frame_time(LOOP_FRAMES));
    }

    #[test]
    fn wash_is_zero_at_seam_and_one_at_cut() {
        assert_eq!(wash(0.0), 0.0);
        assert_eq!(wash(LOOP_SECONDS - 1e-3).round(), 0.0);
        assert!((wash(T_CUT) - 1.0).abs() < 1e-6);
        assert!(wash(T_CUT - 0.01) > 0.99 && wash(T_CUT + 0.01) > 0.99);
    }

    #[test]
    fn separation_starts_at_d0_and_coalesces() {
        assert!((separation(0.0) - D0).abs() < 1e-4);
        assert!(separation(T_COALESCE - 1e-3) < 2.0);
        assert!(separation(T_COALESCE).abs() < 1e-4);
        assert!(separation(T_COALESCE + 20.0).abs() < 1e-3);
    }

    #[test]
    fn bodies_sit_at_centre_of_mass() {
        for t in [0.0, 10.0, 30.0, 40.5, 45.0] {
            let f = frame(t);
            let com: Vec<f32> = (0..3)
                .map(|k| {
                    f.bodies[0].pos[k] * f.bodies[0].mass + f.bodies[1].pos[k] * f.bodies[1].mass
                })
                .collect();
            let total = f.bodies[0].mass + f.bodies[1].mass;
            for c in com {
                assert!((c / total).abs() < 1e-4, "t={t}");
            }
            assert!(
                (dist(f.bodies[0].pos, f.bodies[1].pos) - separation(f.tau).abs()).abs() < 1e-3
            );
        }
    }

    #[test]
    fn orbit_chirps() {
        let early = orbital_rate(separation(1.0));
        let late = orbital_rate(separation(40.0));
        assert!(late > 3.0 * early);
        assert!(orbital_phase(20.0) > orbital_phase(10.0));
    }

    #[test]
    fn camera_never_enters_a_horizon() {
        for n in 0..LOOP_FRAMES {
            let f = frame(frame_time(n));
            for b in f.bodies {
                assert!(dist(f.camera.pos, b.pos) > 1.5 * b.rs, "frame {n}");
            }
        }
    }

    #[test]
    fn camera_path_is_continuous() {
        let mut prev = frame(0.0).camera.pos;
        for n in 1..LOOP_FRAMES {
            let t = frame_time(n);
            let cam = frame(t).camera.pos;
            let step = dist(prev, cam);
            let across_cut = n == (T_CUT * FPS) as u32;
            if !across_cut {
                assert!(step < 0.6, "frame {n} jumped {step}");
            }
            prev = cam;
        }
    }

    #[test]
    fn mini_disks_exist_early_and_vanish_after_merger() {
        assert!(frame(0.0).bodies[0].disk_gain > 0.5);
        assert!(frame(0.0).bodies[0].disk_outer > frame(0.0).bodies[0].disk_inner);
        assert_eq!(frame(50.0).bodies[1].disk_gain, 0.0);
        assert!(frame(50.0).bodies[0].disk_gain > 0.3);
    }
}
