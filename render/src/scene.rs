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
pub const T_COALESCE: f32 = 48.0;
/// The camera has fallen into the remnant's shadow here and the frame is
/// black. The opening fades in from that black, so frame 1440 equals frame 0.
pub const T_CUT: f32 = 58.5;
/// Half-width of the fade, in seconds.
pub const DISSOLVE: f32 = 1.0;
/// Ejecta shell speed, scene units per loop second.
pub const EJECTA_SPEED: f32 = 3.8;

pub const RINGDOWN_AMPLITUDE: f32 = 3.0;
pub const RINGDOWN_OMEGA: f32 = 4.6;
pub const RINGDOWN_TAU: f32 = 2.6;

/// Visual speed of gravitational-wave ripples, scene units per loop second.
pub const GW_SPEED: f32 = 13.0;
/// Strain amplitude at unit distance for the loudest orbit.
pub const GW_AMPLITUDE: f32 = 0.55;
/// The strain table covers this window at this step.
pub const GW_T0: f32 = -14.0;
pub const GW_DT: f32 = 1.0 / 48.0;
pub const GW_LEN: usize = 3700;

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
    /// Merger flash, 0 except in the two seconds after coalescence.
    pub flash: f32,
    pub ejecta: Ejecta,
    pub merged: bool,
}

/// The shell of gas blown out by the merger.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Ejecta {
    pub radius: f32,
    pub brightness: f32,
    pub width: f32,
    /// Seconds since launch; drives the turbulence texture.
    pub age: f32,
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
    const K: f32 = 30.0;
    const MAX: f32 = 7.5;
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

/// Weight of the post-cut scene in the seam dissolve, 0 outside the window.
pub fn dissolve(t: f32) -> f32 {
    smoothstep(
        T_CUT - DISSOLVE,
        T_CUT + DISSOLVE,
        t.rem_euclid(LOOP_SECONDS),
    )
}

/// Merger flash: the shocked mini-disks light up in the first moments after
/// coalescence. Peaks a few frames in, gone within two seconds.
pub fn flash(tau: f32) -> f32 {
    let s = tau - T_COALESCE;
    if !(0.0..2.5).contains(&s) {
        return 0.0;
    }
    smoothstep(0.0, 0.12, s) * (-s / 0.45).exp()
}

/// Camera jolt at the merger: a short decaying rattle, zero elsewhere.
pub fn shake(tau: f32) -> [f32; 3] {
    let s = tau - T_COALESCE;
    if !(0.0..3.5).contains(&s) {
        return [0.0; 3];
    }
    let a = 0.55 * (-s / 0.8).exp() * smoothstep(0.0, 0.08, s);
    [
        a * ((37.0 * s).sin() + 0.5 * (61.0 * s + 1.0).sin()),
        a * ((29.0 * s + 1.0).sin() + 0.5 * (53.0 * s).sin()),
        a * ((43.0 * s + 2.0).sin() + 0.5 * (71.0 * s + 3.0).sin()),
    ]
}

/// Ejecta shell state at physical time `tau`.
pub fn ejecta(tau: f32) -> Ejecta {
    if tau < T_COALESCE {
        return Ejecta::default();
    }
    let s = tau - T_COALESCE;
    Ejecta {
        radius: EJECTA_SPEED * s,
        brightness: 0.6 * (-s / 1.4).exp() + 0.12,
        width: 1.5 + 0.1 * EJECTA_SPEED * s,
        age: s,
    }
}

/// Gravitational-wave strain amplitude at unit distance and phase, as a
/// function of emission time. Twice the orbital phase before coalescence,
/// the quasi-normal ring after it.
pub fn gw_state(t: f32) -> (f32, f32) {
    if t < T_COALESCE {
        let rate = orbital_rate(separation(t));
        let a = GW_AMPLITUDE * (rate / 7.5).powf(2.0 / 3.0);
        (a, 2.0 * orbital_phase(t))
    } else {
        let s = t - T_COALESCE;
        let burst = 1.0 + 2.2 * (-(s / 0.45) * (s / 0.45)).exp();
        let a = GW_AMPLITUDE * burst * (-s / RINGDOWN_TAU).exp();
        (a, 2.0 * orbital_phase(T_COALESCE) + RINGDOWN_OMEGA * s)
    }
}

/// Table of (amplitude, phase, d amplitude/dt, d phase/dt) over the loop and
/// the pre-roll, for the shader's retarded-time lookup.
pub fn gw_table() -> Vec<[f32; 4]> {
    let mut rows = Vec::with_capacity(GW_LEN);
    let mut phase_acc = 0.0f32;
    let mut prev_phase: Option<f32> = None;
    for i in 0..GW_LEN {
        let t = GW_T0 + i as f32 * GW_DT;
        let (a, phi) = gw_state(t);
        // orbital_phase integrates from -4; extend smoothly before that.
        let phi = if t < -4.0 {
            phi + orbital_rate(separation(t)) * 2.0 * (t + 4.0)
        } else {
            phi
        };
        if let Some(p) = prev_phase {
            phase_acc += phi - p;
        }
        prev_phase = Some(phi);
        rows.push([a, phase_acc, 0.0, 0.0]);
    }
    for i in 0..GW_LEN {
        let (lo, hi) = (i.saturating_sub(1), (i + 1).min(GW_LEN - 1));
        let span = (hi - lo) as f32 * GW_DT;
        rows[i][2] = (rows[hi][0] - rows[lo][0]) / span;
        rows[i][3] = (rows[hi][1] - rows[lo][1]) / span;
    }
    rows
}

/// Catmull-Rom keyframes for the camera: (tau, distance, elevation deg, azimuth deg, fov deg).
const CAMERA_KEYS: [[f32; 5]; 12] = [
    [-6.0, 96.0, 9.5, -18.0, 34.0],
    [0.0, 90.0, 9.0, 0.0, 34.0],
    [14.0, 62.0, 8.0, 30.0, 36.0],
    [28.0, 46.0, 7.0, 62.0, 38.0],
    [40.0, 36.0, 8.0, 92.0, 40.0],
    [46.0, 31.0, 12.0, 108.0, 42.0],
    [48.5, 29.0, 15.0, 114.0, 42.0],
    [52.0, 26.0, 26.0, 126.0, 40.0],
    [56.0, 17.0, 32.0, 144.0, 40.0],
    [57.5, 13.5, 35.0, 154.0, 40.0],
    [58.5, 5.5, 36.0, 160.0, 40.0],
    [61.0, 4.0, 36.0, 168.0, 40.0],
];

/// Cubic Hermite with finite-difference tangents in real time, so unevenly
/// spaced keys do not overshoot.
fn camera_key(tau: f32) -> [f32; 4] {
    let n = CAMERA_KEYS.len();
    let tau = tau.clamp(CAMERA_KEYS[0][0], CAMERA_KEYS[n - 1][0]);
    let mut i = 0;
    while i + 2 < n && CAMERA_KEYS[i + 1][0] <= tau {
        i += 1;
    }
    let (k1, k2) = (CAMERA_KEYS[i], CAMERA_KEYS[i + 1]);
    let dt = k2[0] - k1[0];
    let s = ((tau - k1[0]) / dt).clamp(0.0, 1.0);
    let slope = |a: [f32; 5], b: [f32; 5], j: usize| (b[j] - a[j]) / (b[0] - a[0]);
    // Fritsch-Carlson limiting: flat where the slope changes sign, and never
    // more than three times the gentler neighbour.
    let tangent = |idx: usize, j: usize| {
        let here = CAMERA_KEYS[idx];
        match (
            idx.checked_sub(1).map(|p| CAMERA_KEYS[p]),
            CAMERA_KEYS.get(idx + 1),
        ) {
            (Some(prev), Some(next)) => {
                let (a, b) = (slope(prev, here, j), slope(here, *next, j));
                if a * b <= 0.0 {
                    0.0
                } else {
                    let avg = 0.5 * (a + b);
                    avg.signum() * avg.abs().min(3.0 * a.abs().min(b.abs()))
                }
            }
            (None, Some(next)) => slope(here, *next, j),
            (Some(prev), None) => slope(prev, here, j),
            (None, None) => 0.0,
        }
    };
    let (h00, h10, h01, h11) = (
        2.0 * s * s * s - 3.0 * s * s + 1.0,
        s * s * s - 2.0 * s * s + s,
        -2.0 * s * s * s + 3.0 * s * s,
        s * s * s - s * s,
    );
    let mut out = [0.0; 4];
    for (j, o) in out.iter_mut().enumerate() {
        let (m1, m2) = (tangent(i, j + 1) * dt, tangent(i + 1, j + 1) * dt);
        *o = h00 * k1[j + 1] + h10 * m1 + h01 * k2[j + 1] + h11 * m2;
    }
    out
}

pub fn camera(tau: f32) -> Camera {
    let [dist, elev, azim, fov] = camera_key(tau);
    let (el, az) = (elev.to_radians(), azim.to_radians());
    let jolt = shake(tau);
    let pos = [
        dist * el.cos() * az.sin() + jolt[0],
        dist * el.sin() + jolt[1],
        dist * el.cos() * az.cos() + jolt[2],
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
    frame_at(t, physical_time(t))
}

/// Scene state at physical time `tau`, labelled with loop time `t`. The
/// dissolve around the cut renders both sides of the seam this way.
pub fn frame_at(t: f32, tau: f32) -> Frame {
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
            disk_gain: if merged { 0.0 } else { 0.45 * strip },
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
        flash: flash(tau),
        ejecta: ejecta(tau),
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
    fn ejecta_exists_only_after_the_merger() {
        assert_eq!(ejecta(20.0), Ejecta::default());
        assert_eq!(ejecta(-1.0), Ejecta::default());
        let e = ejecta(T_COALESCE + 5.0);
        assert!((e.radius - 5.0 * EJECTA_SPEED).abs() < 1e-4);
        assert!(e.brightness > 0.12);
    }

    #[test]
    fn camera_is_inside_the_shadow_at_the_cut() {
        // Shadow angular radius asin(b_crit / r) must exceed the half diagonal
        // of a 40 degree lens: the frame is black on both sides of the cut.
        for t in [T_CUT - 0.5 * DISSOLVE, T_CUT, T_CUT + DISSOLVE - 1e-3] {
            let f = frame_at(t, t);
            // After the merger both centres carry the remnant's mass.
            let b = &f.bodies[0];
            let r = dist(f.camera.pos, b.pos);
            let b_crit = 2.598 * (f.bodies[0].rs + f.bodies[1].rs);
            let shadow = (b_crit / r).min(1.0).asin().to_degrees();
            assert!(shadow > 24.0, "t={t} shadow {shadow}");
        }
    }

    #[test]
    fn dissolve_is_confined_to_the_seam_window() {
        assert_eq!(dissolve(0.0), 0.0);
        assert_eq!(dissolve(T_CUT - DISSOLVE - 0.1), 0.0);
        assert_eq!(dissolve(T_CUT + DISSOLVE + 0.1), 1.0);
        assert!((dissolve(T_CUT) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn separation_starts_at_d0_and_coalesces() {
        assert!((separation(0.0) - D0).abs() < 1e-4);
        assert!(separation(T_COALESCE - 1e-3) < 2.0);
        assert!(separation(T_COALESCE).abs() < 1e-4);
        assert!(separation(T_COALESCE + 20.0).abs() < 1e-2);
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
        let late = orbital_rate(separation(T_COALESCE - 1.0));
        assert!(late > 3.0 * early);
        assert!(orbital_phase(20.0) > orbital_phase(10.0));
    }

    #[test]
    fn camera_distance_never_overshoots_its_keys() {
        for w in CAMERA_KEYS.windows(2) {
            let (lo, hi) = (w[0][1].min(w[1][1]), w[0][1].max(w[1][1]));
            for k in 0..=20 {
                let t = w[0][0] + (w[1][0] - w[0][0]) * k as f32 / 20.0;
                let d = camera_key(t)[0];
                assert!(
                    d >= lo - 0.3 && d <= hi + 0.3,
                    "t={t} d={d} keys {lo}..{hi}"
                );
            }
        }
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
            let jolting = (T_COALESCE..T_COALESCE + 3.5).contains(&t);
            if !across_cut && !jolting {
                assert!(step < 0.6, "frame {n} jumped {step}");
            }
            prev = cam;
        }
    }

    #[test]
    fn flash_and_shake_only_happen_at_the_merger() {
        assert_eq!(flash(10.0), 0.0);
        assert_eq!(shake(10.0), [0.0; 3]);
        assert_eq!(flash(T_COALESCE - 0.01), 0.0);
        assert!(flash(T_COALESCE + 0.12) > 0.7);
        assert!(flash(T_COALESCE + 2.4) < 0.01);
        assert!(shake(T_COALESCE + 0.2).iter().any(|v| v.abs() > 0.05));
        assert!(shake(T_COALESCE + 3.4).iter().all(|v| v.abs() < 0.02));
    }

    #[test]
    fn gw_table_chirps_and_rings_down() {
        let table = gw_table();
        assert_eq!(table.len(), GW_LEN);
        let at = |t: f32| table[((t - GW_T0) / GW_DT).round() as usize];
        // Phase is monotonic and accelerating toward the merger.
        assert!(at(30.0)[1] > at(10.0)[1]);
        assert!(at(T_COALESCE - 1.0)[3] > 2.0 * at(10.0)[3]);
        // Amplitude peaks at the merger and is gone by the end.
        assert!(at(T_COALESCE + 0.1)[0] > at(35.0)[0]);
        assert!(at(59.0)[0] < 0.01 * at(T_COALESCE)[0]);
        // Finite differences are finite.
        assert!(table.iter().all(|r| r.iter().all(|v| v.is_finite())));
    }

    #[test]
    fn mini_disks_exist_early_and_vanish_after_merger() {
        assert!(frame(0.0).bodies[0].disk_gain > 0.3);
        assert!(frame(0.0).bodies[0].disk_outer > frame(0.0).bodies[0].disk_inner);
        assert_eq!(frame(50.0).bodies[1].disk_gain, 0.0);
        assert!(frame(50.0).bodies[0].disk_gain > 0.3);
    }
}
