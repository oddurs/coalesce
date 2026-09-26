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
/// The loop closes here. The camera runs on loop time, so it is the same on
/// both sides by construction; around the cut it has tilted up and away to
/// the sky, and a dissolve swaps the remnant for the binary out of frame.
pub const T_CUT: f32 = 59.4;
/// Half-width of the dissolve, in seconds.
pub const DISSOLVE: f32 = 0.5;
/// Exposure at rest, shared by both sides of the seam so the grade cannot pop.
pub const EXPOSURE_REST: f32 = 0.72;
/// The opening binary carries the remnant's mass until this physical time and
/// grows to full mass by then, out of frame. Across the dissolve both sides
/// then bend starlight alike; a 5% mass step doubled every star in the mix.
pub const SEAM_MASS_END: f32 = 0.8;
/// The last orbit plays in slow motion: loop time loses this much physical
/// time across the window ending at coalescence.
pub const SLOW_MOTION: f32 = 0.6;
pub const SLOW_MOTION_START: f32 = 46.4;

pub const RINGDOWN_AMPLITUDE: f32 = 3.0;
pub const RINGDOWN_OMEGA: f32 = 4.6;
/// Schwarzschild l=2 ringing has a quality factor near two: the remnant
/// rings a couple of times and goes quiet.
pub const RINGDOWN_TAU: f32 = 1.1;

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
    /// Roll about the view axis, degrees. Positive tilts the horizon clockwise.
    pub roll_deg: f32,
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
    /// Exposure multiplier for the grade: a dark overture that builds.
    pub exposure: f32,
    /// Phase of the two-armed spiral wave the binary drives in the big disk.
    pub spiral_phase: f32,
    /// Phase of the lopsided overdensity orbiting the cavity edge.
    pub lump_phase: f32,
    pub merged: bool,
}

pub fn frame_time(frame: u32) -> f32 {
    (frame % LOOP_FRAMES) as f32 / FPS
}

/// Loop time to physical time. The cut happens at `T_CUT`; after it the
/// opening is playing at negative physical time.
pub fn physical_time(t: f32) -> f32 {
    let t = t.rem_euclid(LOOP_SECONDS);
    if t < T_CUT {
        // Slow motion across the last orbit: physical time falls behind loop
        // time by SLOW_MOTION, smoothly, so coalescence lands a little later.
        let window_end = T_COALESCE + SLOW_MOTION;
        t - SLOW_MOTION * smoothstep(SLOW_MOTION_START, window_end, t)
    } else {
        t - LOOP_SECONDS
    }
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
    const K: f32 = 24.0;
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

/// Exposure arc: a dark overture, a build through the dance, the flash, then
/// the calm and the fall.
pub fn exposure(tau: f32) -> f32 {
    const PEAK: f32 = 1.15;
    const SETTLE: f32 = 0.7;
    let build = EXPOSURE_REST
        + (1.0 - EXPOSURE_REST) * smoothstep(6.0, 34.0, tau)
        + (PEAK - 1.0) * smoothstep(38.0, 47.5, tau);
    let settle = 1.0 - (1.0 - SETTLE) * smoothstep(48.5, 53.0, tau);
    // Lands exactly on the rest value before the seam window opens.
    let fall = 1.0 - (1.0 - EXPOSURE_REST / (PEAK * SETTLE)) * smoothstep(53.5, 56.5, tau);
    build * settle * fall
}

/// Angular rate of the cavity-edge overdensity: Keplerian at the cavity.
pub fn lump_rate(cavity: f32) -> f32 {
    orbital_rate(cavity)
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

/// Camera keyframes on loop time: (t, distance, elevation deg, azimuth deg,
/// fov deg, roll deg, tilt deg). The last key is the first one a loop later,
/// one full turn round, so the path is periodic. Tilt lifts the view off the
/// system toward the sky: the loop closes looking up and away, and opens by
/// tilting back down onto the binary.
const CAMERA_KEYS: [[f32; 7]; 16] = [
    [0.0, 170.0, 14.0, 0.0, 32.0, 0.0, 28.0],
    [0.9, 165.0, 14.0, 5.5, 32.0, 0.0, 22.0],
    [2.4, 150.0, 13.5, 13.5, 32.0, 0.0, 5.0],
    [5.0, 126.0, 12.5, 25.0, 32.5, 0.0, 0.0],
    [7.5, 108.0, 12.0, 34.0, 33.0, 0.0, 0.0],
    [12.0, 88.0, 10.0, 47.0, 34.0, 0.0, 0.0],
    [22.0, 58.0, 7.0, 78.0, 35.0, 0.0, 0.0],
    [34.0, 43.0, 6.5, 132.0, 37.0, 2.0, 0.0],
    [42.0, 34.0, 7.0, 186.0, 39.0, 5.0, 0.0],
    [46.5, 30.0, 9.0, 222.0, 41.0, 8.0, 0.0],
    [48.6, 28.0, 12.0, 244.0, 42.0, 9.0, 0.0],
    [52.0, 27.0, 16.0, 272.0, 40.0, 6.0, 0.0],
    [55.0, 72.0, 16.0, 303.0, 36.0, 2.0, 3.0],
    [57.4, 140.0, 15.0, 334.0, 33.0, 0.5, 10.0],
    [58.6, 162.0, 14.0, 348.0, 32.0, 0.0, 25.0],
    [60.0, 170.0, 14.0, 360.0, 32.0, 0.0, 28.0],
];

/// Key `i`, extended periodically past both ends.
fn key(i: isize) -> [f32; 7] {
    let n = CAMERA_KEYS.len() as isize - 1;
    let laps = i.div_euclid(n);
    let mut k = CAMERA_KEYS[i.rem_euclid(n) as usize];
    k[0] += laps as f32 * LOOP_SECONDS;
    k[3] += laps as f32 * 360.0;
    k
}

/// Periodic cubic Hermite with finite-difference tangents in real time, so
/// unevenly spaced keys do not overshoot and the seam is as smooth as any
/// other instant.
fn camera_key(t: f32) -> [f32; 6] {
    let t = t.rem_euclid(LOOP_SECONDS);
    let mut i = 0;
    while CAMERA_KEYS[i + 1][0] <= t {
        i += 1;
    }
    let i = i as isize;
    let (k1, k2) = (key(i), key(i + 1));
    let dt = k2[0] - k1[0];
    let s = ((t - k1[0]) / dt).clamp(0.0, 1.0);
    let slope = |a: [f32; 7], b: [f32; 7], j: usize| (b[j] - a[j]) / (b[0] - a[0]);
    // Fritsch-Carlson limiting: flat where the slope changes sign, and never
    // more than three times the gentler neighbour.
    let tangent = |idx: isize, j: usize| {
        let (a, b) = (
            slope(key(idx - 1), key(idx), j),
            slope(key(idx), key(idx + 1), j),
        );
        if a * b <= 0.0 {
            0.0
        } else {
            let avg = 0.5 * (a + b);
            avg.signum() * avg.abs().min(3.0 * a.abs().min(b.abs()))
        }
    };
    let (h00, h10, h01, h11) = (
        2.0 * s * s * s - 3.0 * s * s + 1.0,
        s * s * s - 2.0 * s * s + s,
        -2.0 * s * s * s + 3.0 * s * s,
        s * s * s - s * s,
    );
    let mut out = [0.0; 6];
    for (j, o) in out.iter_mut().enumerate() {
        let (m1, m2) = (tangent(i, j + 1) * dt, tangent(i + 1, j + 1) * dt);
        *o = h00 * k1[j + 1] + h10 * m1 + h01 * k2[j + 1] + h11 * m2;
    }
    out
}

/// Organic drift, a few slow sines. Scaled with distance so it stays a
/// fraction of a degree on screen, and grows a little as the dance tightens.
pub fn sway(t: f32, dist: f32) -> [f32; 3] {
    let gate = smoothstep(2.0, 6.0, t) * (1.0 - smoothstep(52.0, 56.0, t));
    let a = 0.012 * dist * (1.0 + 0.6 * smoothstep(30.0, 47.0, t)) * gate;
    [
        a * ((0.37 * t + 1.0).sin() + 0.5 * (0.91 * t).sin()),
        a * 0.6 * ((0.29 * t).sin() + 0.5 * (0.73 * t + 2.0).sin()),
        a * ((0.41 * t + 2.0).sin() + 0.5 * (0.83 * t + 1.0).sin()),
    ]
}

pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where the camera looks at the system from.
pub const TARGET: [f32; 3] = [0.0, 0.4, 0.0];

/// Camera at loop time `t`. The path runs on loop time so it is periodic; only
/// the merger jolt follows physical time `tau`.
pub fn camera(t: f32, tau: f32) -> Camera {
    let [dist, elev, azim, fov, roll, tilt] = camera_key(t);
    let t = t.rem_euclid(LOOP_SECONDS);
    let (el, az) = (elev.to_radians(), azim.to_radians());
    let jolt = shake(tau);
    let drift = sway(t, dist);
    let pos = [
        dist * el.cos() * az.sin() + jolt[0] + drift[0],
        dist * el.sin() + jolt[1] + drift[1],
        dist * el.cos() * az.cos() + jolt[2] + drift[2],
    ];
    // Tilt the view up off the system, about the camera's own right axis. The
    // up vector tilts with it, so looking near the zenith stays well defined.
    let fwd0 = normalize([TARGET[0] - pos[0], TARGET[1] - pos[1], TARGET[2] - pos[2]]);
    let right = normalize(cross(fwd0, [0.0, 1.0, 0.0]));
    let up0 = cross(right, fwd0);
    let (st, ct) = tilt.to_radians().sin_cos();
    let fwd = [0, 1, 2].map(|k| fwd0[k] * ct + up0[k] * st);
    let up = [0, 1, 2].map(|k| up0[k] * ct - fwd0[k] * st);
    Camera {
        pos,
        look_at: [pos[0] + fwd[0], pos[1] + fwd[1], pos[2] + fwd[2]],
        up,
        roll_deg: roll,
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
        let m = 1.0 - MERGER_LOSS * (1.0 - smoothstep(0.0, SEAM_MASS_END, tau));
        [m * M1, m * M2]
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
        // Tidal heating: the closer the pair, the harder the disks are stirred.
        let squeeze = (1.0 - d.abs() / D0).clamp(0.0, 1.0);
        let heat = 1.0 + 1.5 * squeeze * squeeze;
        bodies[i] = Body {
            pos: [offsets[i] * dir[0], 0.0, offsets[i] * dir[2]],
            vel: [speed * tangent[0], 0.0, speed * tangent[2]],
            mass: m,
            rs,
            disk_inner,
            disk_outer,
            disk_gain: if merged { 0.0 } else { 0.45 * strip * heat },
            disk_temp: 5200.0 + 1600.0 * squeeze,
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
        // Shocked disk gas flashes white-hot and cools back to amber.
        let f = flash(tau);
        bodies[0].disk_gain = 0.5 * smoothstep(0.0, 2.5, s) + 2.5 * f;
        bodies[0].disk_temp = 5000.0 + 3500.0 * f;
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
        camera: camera(t, tau),
        flash: flash(tau),
        exposure: exposure(tau),
        spiral_phase: phi,
        lump_phase: lump_rate(circumbinary_inner) * tau,
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
    fn slow_motion_holds_the_last_orbit() {
        // Physical time never runs backwards and never below half speed.
        let mut prev = physical_time(0.0);
        for n in 1..(T_CUT * FPS) as u32 {
            let tau = physical_time(n as f32 / FPS);
            let rate = (tau - prev) * FPS;
            assert!(rate > 0.35 && rate <= 1.001, "frame {n} rate {rate}");
            prev = tau;
        }
        // Coalescence lands later in loop time than in physical time.
        assert!(physical_time(T_COALESCE + SLOW_MOTION) - T_COALESCE < 1e-3);
        assert!(physical_time(T_COALESCE) < T_COALESCE);
    }

    #[test]
    fn exposure_arc_builds_to_the_merger() {
        assert!(exposure(0.0) < exposure(30.0));
        assert!(exposure(30.0) < exposure(47.5));
        assert!(exposure(52.0) < exposure(47.5));
        for t in 0..60 {
            let e = exposure(t as f32);
            assert!(e > 0.5 && e < 1.5);
        }
    }

    fn angle_deg(a: [f32; 3], b: [f32; 3]) -> f32 {
        let (a, b) = (normalize(a), normalize(b));
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    }

    fn velocity(t: f32) -> [f32; 3] {
        let h = 1.0 / FPS;
        let (a, b) = (camera(t - h, 0.0).pos, camera(t + h, 0.0).pos);
        [0, 1, 2].map(|k| (b[k] - a[k]) / (2.0 * h))
    }

    #[test]
    fn camera_is_periodic_and_smooth_across_the_wrap() {
        let (a, b) = (camera(0.0, 0.0), camera(LOOP_SECONDS, 0.0));
        assert!(dist(a.pos, b.pos) < 1e-3, "{a:?} {b:?}");
        assert!(dist(a.look_at, b.look_at) < 1e-3);
        // Velocity just before the wrap matches just after it: no kick.
        let (v0, v1) = (velocity(LOOP_SECONDS - 0.01), velocity(0.01));
        assert!(dist(v0, v1) < 0.02 * dist(v0, [0.0; 3]), "{v0:?} {v1:?}");
    }

    /// Angle in degrees by which the system's outer disk clears the frame;
    /// negative when any of it is in view. Measured to the frame's edges.
    fn clearance_deg(c: &Camera) -> f32 {
        let fwd = normalize([0, 1, 2].map(|k| c.look_at[k] - c.pos[k]));
        let right = normalize(cross(fwd, c.up));
        let up = cross(right, fwd);
        let tan_x = (c.fov_x_deg * 0.5).to_radians().tan();
        let tan_y = tan_x * 2160.0 / 4096.0;
        let to_system = [0, 1, 2].map(|k| TARGET[k] - c.pos[k]);
        let disk = (32.0 / dist(c.pos, TARGET)).atan().to_degrees();
        let mut nearest = f32::MAX;
        for i in 0..=64 {
            let u = i as f32 / 32.0 - 1.0;
            for (x, y) in [(u, -1.0), (u, 1.0), (-1.0, u), (1.0, u)] {
                let d = [0, 1, 2].map(|k| fwd[k] + right[k] * x * tan_x + up[k] * y * tan_y);
                nearest = nearest.min(angle_deg(d, to_system));
            }
        }
        let inside = {
            let z = to_system.iter().zip(fwd).map(|(a, b)| a * b).sum::<f32>();
            let x = to_system.iter().zip(right).map(|(a, b)| a * b).sum::<f32>() / z;
            let y = to_system.iter().zip(up).map(|(a, b)| a * b).sum::<f32>() / z;
            z > 0.0 && x.abs() < tan_x && y.abs() < tan_y
        };
        if inside {
            -nearest - disk
        } else {
            nearest - disk
        }
    }

    #[test]
    fn system_is_out_of_frame_through_the_dissolve() {
        // The remnant turns back into the binary out of view: over the whole
        // window the outer disk sits clear below the bottom of the frame.
        let mut t = T_CUT - DISSOLVE;
        while t <= T_CUT + DISSOLVE {
            let margin = clearance_deg(&camera(t, 0.0));
            assert!(margin > 4.0, "t={t} margin {margin} deg");
            t += 0.05;
        }
    }

    #[test]
    fn both_sides_of_the_seam_weigh_the_same() {
        let total = |f: Frame| f.bodies[0].mass + f.bodies[1].mass;
        let mut t = T_CUT - DISSOLVE;
        while t <= T_CUT + DISSOLVE {
            let before = total(frame_at(t, t));
            let after = total(frame_at(t, t - LOOP_SECONDS));
            assert!((before - after).abs() < 1e-6, "t={t} {before} vs {after}");
            t += 0.125;
        }
    }

    #[test]
    fn binary_is_at_full_mass_before_it_is_in_frame() {
        let f = frame(SEAM_MASS_END);
        assert!((f.bodies[0].mass + f.bodies[1].mass - 1.0).abs() < 1e-6);
        assert!(clearance_deg(&f.camera) > 0.0);
    }

    #[test]
    fn exposure_does_not_jump_at_the_cut() {
        let e = |t: f32| exposure(physical_time(t));
        assert!((e(T_CUT - 1e-3) - e(T_CUT + 1e-3)).abs() < 1e-4);
        assert!((e(LOOP_SECONDS - 1e-3) - e(0.0)).abs() < 1e-4);
    }

    #[test]
    fn dissolve_is_confined_to_the_seam_window() {
        assert_eq!(dissolve(0.0), 0.0);
        assert_eq!(dissolve(T_CUT - DISSOLVE - 0.1), 0.0);
        // The window closes at the wrap; the opening takes over from there.
        assert!(dissolve(T_CUT + DISSOLVE - 1e-3) > 0.999);
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
            let jolting = (T_COALESCE..T_COALESCE + 3.5).contains(&t);
            if !jolting {
                let limit = 0.6 + 0.15 * frame(t).camera.focus_distance;
                assert!(step < limit, "frame {n} jumped {step}");
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
