//! The loop timeline: where the black holes are, where the camera is, and how
//! bright the light wash is, all as pure functions of loop time.
//!
//! The loop is scored: the song starts at frame 0 and the story follows it.
//! The orbit is timed off the song's pulse, each pass of the two holes landing
//! on its eighth-note grid, and the merger lands on the downbeat where the
//! song's riser peaks. After the song, two bars of silence carry the camera
//! back to the stars and round to the opening.
//!
//! Units: G = c = 1 and the total mass is 1, so the merged Schwarzschild radius
//! is 2. One loop second is not one geometric time unit; orbital rates are
//! scaled for the eye.

pub const FPS: f32 = 24.0;
/// The song runs this long from frame 0; the loop holds two quiet bars after it.
pub const SONG_SECONDS: f32 = 213.6;
pub const LOOP_SECONDS: f32 = 220.0;
pub const LOOP_FRAMES: u32 = 5280;
/// Shutter open time: 180 degrees, half a frame, as film is shot.
pub const SHUTTER: f32 = 0.5 / FPS;

/// The song's pulse: 75 beats a minute, four to the bar, the first downbeat
/// at `BAR0`. Measured from the track, not assumed.
pub const BEAT: f32 = 0.8;
pub const BAR: f32 = 4.0 * BEAT;
pub const BAR0: f32 = 1.666;
/// The pair's passes are never closer than an eighth note.
pub const CLOSEST: f32 = 0.5 * BEAT;

/// Masses of the two bodies. They sum to 1.
pub const M1: f32 = 0.58;
pub const M2: f32 = 0.42;
/// Mass radiated away as gravitational waves at merger.
pub const MERGER_LOSS: f32 = 0.05;

/// Nominal separation of the opening orbit, for scaling the tidal heating.
pub const D0: f32 = 12.0;
/// Coalescence: the downbeat of bar 54, where the song's riser peaks.
pub const T_COALESCE: f32 = BAR0 + 54.0 * BAR;
/// The loop closes here, a bar into the silence. The camera runs on loop time,
/// so it is the same on both sides by construction; around the cut it has
/// tilted up and away to the sky, and a dissolve swaps the remnant for the
/// binary out of frame.
pub const T_CUT: f32 = SONG_SECONDS + BAR;
/// Half-width of the dissolve, in seconds.
pub const DISSOLVE: f32 = 2.4;
// The music is over before the dissolve starts, so the seam is in silence, and
// the dissolve is done by the wrap.
const _: () = assert!(SONG_SECONDS < T_CUT - DISSOLVE && T_CUT + DISSOLVE <= LOOP_SECONDS);
const _: () = assert!(LOOP_FRAMES as f32 == LOOP_SECONDS * FPS);
/// Exposure at rest, shared by both sides of the seam so the grade cannot pop.
pub const EXPOSURE_REST: f32 = 0.72;
/// The opening binary carries the remnant's mass until this physical time and
/// grows to full mass by then, out of frame. Across the dissolve both sides
/// then bend starlight alike; a 5% mass step doubled every star in the mix.
pub const SEAM_MASS_END: f32 = 3.0;

pub const RINGDOWN_AMPLITUDE: f32 = 3.0;
pub const RINGDOWN_OMEGA: f32 = 4.6;
/// Schwarzschild l=2 ringing has a quality factor near two: the remnant
/// rings a couple of times and goes quiet.
pub const RINGDOWN_TAU: f32 = 1.1;

/// Kepler's constant for the orbit, scaled for the eye: the opening passes,
/// one per two-bar phrase, sit about twelve units apart.
const KEPLER: f32 = 20.4;
/// The last stretch before coalescence, when the pair plunges together.
const PLUNGE: f32 = 1.2;
/// Closest the pair comes before the plunge. Their shadows just touch here,
/// so the fastest passes read as two holes whirling round each other rather
/// than one wobbling shadow.
const SPIN_FLOOR: f32 = 4.2;

/// Visual speed of gravitational-wave ripples, scene units per loop second.
pub const GW_SPEED: f32 = 13.0;
/// Strain amplitude at unit distance for the loudest orbit.
pub const GW_AMPLITUDE: f32 = 0.55;
/// The strain table covers this window at this step: the loop and enough
/// pre-roll for the retarded time of the farthest ray.
pub const GW_T0: f32 = -30.0;
pub const GW_DT: f32 = 1.0 / 48.0;
pub const GW_LEN: usize = ((LOOP_SECONDS - GW_T0) / GW_DT) as usize + 2;

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
    /// How much the disk warps, flares and frays, 0 to 1: only the remnant's,
    /// as its gas settles after the merger.
    pub disk_life: f32,
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
    /// Loop time in seconds, 0 <= t < LOOP_SECONDS.
    pub t: f32,
    /// Physical scene time. Negative after the cut, when the opening shot is
    /// already on screen under the dissolve.
    pub tau: f32,
    pub bodies: [Body; 2],
    pub circumbinary_inner: f32,
    pub circumbinary_outer: f32,
    pub circumbinary_gain: f32,
    pub camera: Camera,
    /// Merger flash, 0 except in the moments after coalescence.
    pub flash: f32,
    /// Exposure multiplier for the grade: a dark overture that builds.
    pub exposure: f32,
    /// Phase of the two-armed spiral wave the binary drives in the big disk.
    pub spiral_phase: f32,
    /// Phase of the lopsided overdensity orbiting the cavity edge.
    pub lump_phase: f32,
    /// The sky's brightness and colour, 1 at rest: drained as the pair
    /// tightens, flooding back with the flash.
    pub sky_gain: f32,
    pub sky_saturation: f32,
    /// Crackle in the hottest gas of the mini-disks, surging on each eclipse.
    pub electric: f32,
    pub merged: bool,
}

pub fn frame_time(frame: u32) -> f32 {
    (frame % LOOP_FRAMES) as f32 / FPS
}

/// `n` instants stratified across the shutter, centred on `t`. Tracing one
/// sample per instant blurs motion the way an open shutter does.
pub fn shutter_times(t: f32, n: u32) -> Vec<f32> {
    (0..n)
        .map(|i| t + ((i as f32 + 0.5) / n as f32 - 0.5) * SHUTTER)
        .collect()
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Quintic ease: zero velocity and zero acceleration at both ends.
fn smootherstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// How fast physical time runs at loop time `t`. Real time up to the merger,
/// so every pass stays on the beat; the blast plays at a third of real speed
/// through the song's bright sustain, and time eases back as it settles.
pub fn playback_rate(t: f32) -> f32 {
    let mix = |a: f32, b: f32, k: f32| a + (b - a) * k;
    let m = T_COALESCE;
    let mut r = mix(1.0, 0.3, smoothstep(m, m + 0.25, t));
    r = mix(r, 0.65, smoothstep(m + 4.0, m + 7.0, t));
    mix(r, 1.0, smoothstep(m + 10.0, m + 14.0, t))
}

const WARP_DT: f32 = 1.0 / 480.0;

/// Physical time at each step of loop time: the playback rate, integrated.
fn warp() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let n = (LOOP_SECONDS / WARP_DT) as usize + 2;
        // Summed in f64: a hundred thousand f32 steps drift by milliseconds.
        let mut tau = 0.0f64;
        (0..n)
            .map(|i| {
                let at = tau as f32;
                tau += (playback_rate((i as f32 + 0.5) * WARP_DT) * WARP_DT) as f64;
                at
            })
            .collect()
    })
}

/// Loop time to physical time. The cut happens at `T_CUT`; after it the
/// opening is playing at negative physical time.
pub fn physical_time(t: f32) -> f32 {
    let t = t.rem_euclid(LOOP_SECONDS);
    if t >= T_CUT {
        return t - LOOP_SECONDS;
    }
    if t <= T_COALESCE {
        return t;
    }
    let table = warp();
    let f = t / WARP_DT;
    let i = (f as usize).min(table.len() - 2);
    table[i] + (table[i + 1] - table[i]) * (f - i as f32)
}

/// The song's landmarks the pair eclipses on, as (bar, passes counted back
/// from coalescence). A pass is half a turn; at each one the far hole lines up
/// behind the near one as the camera sees it and its light bends into a ring.
/// The pair eclipses on the first big swell, on the hits every other bar
/// through the song's body, a pass a bar through bars 38 to 46, sixteen more
/// through the last build and six in the final bar.
const ECLIPSES: [(f32, f32); 9] = [
    (4.0, -53.0),
    (22.0, -43.0),
    (38.0, -30.0),
    (40.0, -28.0),
    (42.0, -26.0),
    (44.0, -24.0),
    (46.0, -22.0),
    (53.0, -6.0),
    (54.0, 0.0),
];

/// Passes counted over `span` seconds while the gap between them shrinks
/// geometrically from `g0` to `g1`.
fn passes_over(span: f32, g0: f32, g1: f32) -> f32 {
    let r = g1 / g0;
    if (r - 1.0).abs() < 1e-5 {
        span / g0
    } else {
        span / g0 * (1.0 - 1.0 / r) / r.ln()
    }
}

/// Anchor times, pass counts, and the gap between passes at each anchor:
/// solved back from an eighth note at coalescence so each stretch holds its
/// count of passes.
fn eclipses() -> &'static [(f32, f32, f32)] {
    static TABLE: std::sync::OnceLock<Vec<(f32, f32, f32)>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let n = ECLIPSES.len();
        let mut out = vec![(0.0, 0.0, 0.0); n];
        let at = |i: usize| BAR0 + ECLIPSES[i].0 * BAR;
        out[n - 1] = (at(n - 1), ECLIPSES[n - 1].1, CLOSEST);
        for i in (0..n - 1).rev() {
            let (span, count, g1) = (
                at(i + 1) - at(i),
                ECLIPSES[i + 1].1 - ECLIPSES[i].1,
                out[i + 1].2,
            );
            // More passes need smaller gaps: bisect on the gap at the start.
            let (mut lo, mut hi) = (1e-3f32, 100.0f32);
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if passes_over(span, mid, g1) > count {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out[i] = (at(i), ECLIPSES[i].1, 0.5 * (lo + hi));
        }
        out
    })
}

/// Passes made by time `tau`, counted back from coalescence, and passes per
/// second. The gap between passes shrinks geometrically between anchors, so
/// the rate is continuous; before the first anchor it holds steady.
fn pass_count(tau: f32) -> (f32, f32) {
    let e = eclipses();
    let (t0, n0, g0) = e[0];
    if tau <= t0 {
        return (n0 - (t0 - tau) / g0, 1.0 / g0);
    }
    let last = e[e.len() - 1];
    if tau >= last.0 {
        return (last.1, 1.0 / last.2);
    }
    let i = e.partition_point(|a| a.0 <= tau) - 1;
    let ((ta, na, ga), (tb, _, gb)) = (e[i], e[i + 1]);
    let span = tb - ta;
    let x = (tau - ta) / span;
    let r = gb / ga;
    let gap = ga * r.powf(x);
    (na + passes_over(x * span, ga, ga * r.powf(x)), 1.0 / gap)
}

/// The camera's azimuth in radians, unwrapped across loops so it is continuous.
fn azimuth(t: f32) -> f32 {
    camera_key(t)[2].to_radians() + std::f32::consts::TAU * (t / LOOP_SECONDS).floor()
}

/// Orbital angular rate in radians per loop second: half a turn per pass.
/// The camera's own slow turn adds a little to what the eye sees.
pub fn orbital_rate(tau: f32) -> f32 {
    std::f32::consts::PI * pass_count(tau.min(T_COALESCE)).1
}

/// Keplerian angular rate at radius `d` on the orbit's scale.
pub fn kepler_rate(d: f32) -> f32 {
    KEPLER * d.abs().max(0.5).powf(-1.5)
}

/// Separation of the two bodies: Kepler's radius for the orbit's rate, held
/// at `SPIN_FLOOR` through the fastest passes, with a last plunge to contact,
/// then a damped quasi-normal wobble.
pub fn separation(tau: f32) -> f32 {
    if tau < T_COALESCE {
        let kepler = (KEPLER / orbital_rate(tau)).powf(2.0 / 3.0).max(SPIN_FLOOR);
        kepler * ((T_COALESCE - tau) / PLUNGE).clamp(0.0, 1.0).powf(0.25)
    } else {
        let s = tau - T_COALESCE;
        RINGDOWN_AMPLITUDE * (-s / RINGDOWN_TAU).exp() * (RINGDOWN_OMEGA * s).sin()
    }
}

/// Orbital phase: half a turn per pass, measured from the line to the camera,
/// so at every whole pass the two holes line up with it.
pub fn orbital_phase(tau: f32) -> f32 {
    let t = tau.min(T_COALESCE);
    let (count, _) = pass_count(t);
    // Body 0 sits along (cos phi, sin phi) in the orbital plane; the camera at
    // azimuth `az` looks from (sin az, cos az).
    let mut phi = std::f32::consts::PI * count + std::f32::consts::FRAC_PI_2 - azimuth(t);
    if tau > T_COALESCE {
        // Ringdown wobbles along the axis where the merger happened.
        phi += 0.6 * (tau - T_COALESCE).min(1.0);
    }
    phi
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
/// coalescence. Peaks within a frame of the downbeat, gone within two seconds.
pub fn flash(tau: f32) -> f32 {
    let s = tau - T_COALESCE;
    if !(0.0..2.5).contains(&s) {
        return 0.0;
    }
    smoothstep(0.0, 0.04, s) * (-s / 0.45).exp()
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

/// The pull of the tightening pair on the camera: a shudder on every pass,
/// growing through the song's build to the merger, as a fraction of the
/// camera's distance.
pub fn rumble(tau: f32) -> [f32; 3] {
    if !(T_COALESCE - 30.0..T_COALESCE).contains(&tau) {
        return [0.0; 3];
    }
    let build = smoothstep(T_COALESCE - 30.0, T_COALESCE, tau);
    let strain = (orbital_rate(tau) * CLOSEST / std::f32::consts::PI).powf(2.0 / 3.0);
    let a = 0.006 * build * build * strain;
    let wave = 2.0 * orbital_phase(tau);
    [
        a * ((wave).sin() + 0.35 * (2.9 * wave + 1.0).sin()),
        a * 0.6 * ((wave + 1.3).sin() + 0.35 * (3.7 * wave).sin()),
        a * ((wave + 2.4).sin() + 0.35 * (2.3 * wave + 2.0).sin()),
    ]
}

/// Exposure arc: a dark overture, a build through the song to the flash, the
/// calm of the ringdown, and the fall to rest as the music fades out.
pub fn exposure(tau: f32) -> f32 {
    const PEAK: f32 = 1.15;
    const SETTLE: f32 = 0.7;
    let m = T_COALESCE;
    let build = EXPOSURE_REST
        + (1.0 - EXPOSURE_REST) * smoothstep(12.0, 70.0, tau)
        + (PEAK - 1.0) * smoothstep(m - 26.0, m - 0.5, tau);
    let settle = 1.0 - (1.0 - SETTLE) * smoothstep(m + 0.3, m + 3.5, tau);
    // Lands exactly on the rest value before the seam window opens.
    let fall = 1.0 - (1.0 - EXPOSURE_REST / (PEAK * SETTLE)) * smoothstep(m + 20.5, m + 30.5, tau);
    build * settle * fall
}

/// The sky's life, as (brightness, saturation). Through the build the stars
/// and nebula dim and grey, as if the merger drew the light out of them; with
/// the flash they flood back past their rest and settle.
pub fn sky_life(tau: f32) -> (f32, f32) {
    const DRAINED: (f32, f32) = (0.45, 0.4);
    let s = tau - T_COALESCE;
    if s < 0.0 {
        let k = smoothstep(-26.0, -0.3, s);
        (1.0 - (1.0 - DRAINED.0) * k * k, 1.0 - (1.0 - DRAINED.1) * k)
    } else {
        let back = smoothstep(0.0, 0.15, s);
        let gain = DRAINED.0 + (1.0 + 0.25 * (-s / 1.2).exp() - DRAINED.0) * back;
        (
            gain,
            DRAINED.1 + (1.0 - DRAINED.1) * smoothstep(0.0, 0.3, s),
        )
    }
}

/// How much the mini-disks' hottest gas crackles: a low idle, a surge on
/// each eclipse, which the song's pulse times, growing through the build.
pub fn electric(tau: f32) -> f32 {
    if tau >= T_COALESCE {
        return 0.0;
    }
    let (count, _) = pass_count(tau);
    let off = count - count.round();
    let surge = (-(off * off) / (2.0 * 0.07 * 0.07)).exp();
    let build = smoothstep(T_COALESCE - 30.0, T_COALESCE, tau);
    (0.3 + 0.7 * surge) * (0.6 + 0.4 * build)
}

/// Angular rate of the cavity-edge overdensity: Keplerian at the cavity.
pub fn lump_rate(cavity: f32) -> f32 {
    kepler_rate(cavity)
}

/// Gravitational-wave strain amplitude at unit distance and phase, as a
/// function of emission time. Twice the orbital phase before coalescence,
/// the quasi-normal ring after it.
pub fn gw_state(t: f32) -> (f32, f32) {
    if t < T_COALESCE {
        let rate = orbital_rate(t);
        let a = GW_AMPLITUDE * (rate * CLOSEST / std::f32::consts::PI).powf(2.0 / 3.0);
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
    let mut rows: Vec<[f32; 4]> = (0..GW_LEN)
        .map(|i| {
            let (a, phi) = gw_state(GW_T0 + i as f32 * GW_DT);
            [a, phi, 0.0, 0.0]
        })
        .collect();
    for i in 0..GW_LEN {
        let (lo, hi) = (i.saturating_sub(1), (i + 1).min(GW_LEN - 1));
        let span = (hi - lo) as f32 * GW_DT;
        rows[i][2] = (rows[hi][0] - rows[lo][0]) / span;
        rows[i][3] = (rows[hi][1] - rows[lo][1]) / span;
    }
    rows
}

/// Camera keyframes on loop time: (t, distance, elevation deg, azimuth deg,
/// fov deg, roll deg). The last key is the first one a loop later, one full
/// turn round, so the path is periodic. Keys sit on the song's landmarks. The
/// tilt to the sky and back is its own eased curve, [`tilt`].
const CAMERA_KEYS: [[f32; 6]; 28] = [
    [0.0, 170.0, 14.0, 0.0, 32.0, 0.0],
    [4.0, 168.0, 13.9, 4.0, 32.0, 0.0],
    [9.0, 157.0, 13.6, 12.0, 32.0, 0.0],
    // The first big swell: the pair arrives in frame.
    [14.5, 136.0, 13.0, 21.0, 32.2, 0.0],
    [20.0, 113.0, 12.0, 31.0, 33.0, 0.0],
    // The body of the song comes in.
    [27.0, 90.0, 10.0, 44.0, 34.0, 0.0],
    [40.0, 68.0, 6.5, 70.0, 35.0, 0.5],
    [55.0, 56.0, 3.0, 99.0, 35.5, 1.5],
    // Edge on for the eclipse on bar 22: the disks' light bends over and
    // under the far hole.
    [72.0, 50.0, 1.2, 129.0, 36.0, 2.5],
    // Down under the disk's plane...
    [84.0, 47.0, -2.0, 149.0, 36.5, 3.0],
    // ...and up over it as the song lifts and brightens.
    [97.5, 49.0, 7.5, 166.0, 36.5, 1.5],
    [110.0, 53.0, 13.0, 182.0, 36.0, 0.5],
    // The bass drops out and slams back.
    [126.5, 47.0, 8.5, 197.0, 37.0, 2.0],
    [129.7, 44.5, 7.0, 201.0, 37.0, 2.5],
    [142.5, 40.0, 4.0, 218.0, 38.0, 4.0],
    // The held breath. From here to the merger the camera pushes in as the
    // lens widens, the pair held the same size in frame while the world
    // around it stretches: tan(fov / 2) * distance stays constant.
    [148.5, 38.0, 3.5, 226.0, 38.5, 5.0],
    [156.0, 33.0, 5.0, 239.0, 43.8, 6.0],
    [164.0, 29.0, 7.5, 253.0, 49.1, 7.5],
    [170.0, 26.5, 10.0, 264.0, 53.1, 8.5],
    // The merger, at the widest.
    [174.5, 25.0, 12.0, 272.0, 55.9, 9.0],
    // The lens snaps back and the camera is thrown clear: the jolt.
    [175.4, 26.0, 13.0, 275.0, 45.0, 8.0],
    [178.0, 26.5, 14.5, 282.0, 41.5, 7.0],
    [188.0, 27.0, 17.0, 303.0, 39.0, 4.5],
    [199.0, 33.0, 16.5, 321.0, 37.5, 2.5],
    // The song fades; the camera lets go of the remnant.
    [204.0, 66.0, 15.5, 334.0, 35.0, 1.0],
    [209.0, 128.0, 14.8, 345.0, 33.0, 0.3],
    [214.0, 163.0, 14.2, 354.0, 32.0, 0.0],
    [220.0, 170.0, 14.0, 360.0, 32.0, 0.0],
];

/// How far the view is tilted up off the system toward the sky, degrees.
pub const TILT_SKY: f32 = 25.0;

/// The tilt: up and away to the sky as the song fades, and back down onto
/// the pair on the first big swell. A quintic ease at both ends, so each pan
/// starts and lands without a jerk.
pub fn tilt(t: f32) -> f32 {
    let t = t.rem_euclid(LOOP_SECONDS);
    TILT_SKY * (1.0 - smootherstep(0.0, 14.5, t) + smootherstep(201.0, 216.5, t))
}

/// Key `i`, extended periodically past both ends.
fn key(i: isize) -> [f32; 6] {
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
fn camera_key(t: f32) -> [f32; 5] {
    let t = t.rem_euclid(LOOP_SECONDS);
    let mut i = 0;
    while CAMERA_KEYS[i + 1][0] <= t {
        i += 1;
    }
    let i = i as isize;
    let (k1, k2) = (key(i), key(i + 1));
    let dt = k2[0] - k1[0];
    let s = ((t - k1[0]) / dt).clamp(0.0, 1.0);
    let slope = |a: [f32; 6], b: [f32; 6], j: usize| (b[j] - a[j]) / (b[0] - a[0]);
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
    let mut out = [0.0; 5];
    for (j, o) in out.iter_mut().enumerate() {
        let (m1, m2) = (tangent(i, j + 1) * dt, tangent(i + 1, j + 1) * dt);
        *o = h00 * k1[j + 1] + h10 * m1 + h01 * k2[j + 1] + h11 * m2;
    }
    out
}

/// Organic drift, a few slow sines. Scaled with distance so it stays a
/// fraction of a degree on screen, and grows a little as the dance tightens.
/// Still through the opening tilt and the closing pull back to the sky.
pub fn sway(t: f32, dist: f32) -> [f32; 3] {
    let gate = smoothstep(12.0, 20.0, t) * (1.0 - smoothstep(199.0, 206.0, t));
    let a = 0.012 * dist * (1.0 + 0.6 * smoothstep(120.0, 170.0, t)) * gate;
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
/// the rumble of the build and the merger jolt follow physical time `tau`.
pub fn camera(t: f32, tau: f32) -> Camera {
    let [dist, elev, azim, fov, roll] = camera_key(t);
    let t = t.rem_euclid(LOOP_SECONDS);
    let tilt = tilt(t);
    let (el, az) = (elev.to_radians(), azim.to_radians());
    let jolt = shake(tau);
    let drift = sway(t, dist);
    let pull = rumble(tau).map(|v| v * dist);
    let pos = [
        dist * el.cos() * az.sin() + jolt[0] + drift[0] + pull[0],
        dist * el.sin() + jolt[1] + drift[1] + pull[1],
        dist * el.cos() * az.cos() + jolt[2] + drift[2] + pull[2],
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
        disk_life: 0.0,
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
        // The disks burn to the end of the plunge: the song's riser climbs to
        // the merger, and the frame must not go dark under it.
        let strip = smoothstep(1.0, 3.0, d.abs());
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
            disk_life: 0.0,
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
        bodies[0].disk_gain = 0.5 * smoothstep(0.0, 2.5, s) + 0.6 * f;
        bodies[0].disk_life = smoothstep(0.0, 3.0, s);
        bodies[0].disk_temp = 5000.0 + 2000.0 * f;
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
        sky_gain: sky_life(tau).0,
        sky_saturation: sky_life(tau).1,
        electric: electric(tau),
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
    fn shutter_is_centred_and_half_a_frame_wide() {
        let ts = shutter_times(10.0, 16);
        assert_eq!(ts.len(), 16);
        let mean = ts.iter().sum::<f32>() / 16.0;
        assert!((mean - 10.0).abs() < 1e-5);
        let span = ts[15] - ts[0];
        assert!(span < SHUTTER && span > 0.9 * SHUTTER);
        assert_eq!(shutter_times(10.0, 1), vec![10.0]);
    }

    /// Loop time at which physical time first reaches `tau`.
    fn loop_time_of(tau: f32) -> f32 {
        let mut t = 0.0;
        while physical_time(t) < tau {
            t += 1.0 / 480.0;
        }
        t
    }

    #[test]
    fn slow_motion_holds_the_merger() {
        // Physical time never runs backwards and never below a quarter speed.
        let mut prev = physical_time(0.0);
        for n in 1..(T_CUT * FPS) as u32 {
            let tau = physical_time(n as f32 / FPS);
            let rate = (tau - prev) * FPS;
            assert!(rate > 0.25 && rate <= 1.001, "frame {n} rate {rate}");
            prev = tau;
        }
        // The dance runs in real time; the flash plays over several seconds.
        assert!((physical_time(140.0) - 140.0).abs() < 1e-3);
        let merge = loop_time_of(T_COALESCE);
        let cooled = loop_time_of(T_COALESCE + 1.5);
        assert!(cooled - merge > 4.0, "flash spans {} s", cooled - merge);
        // The ringdown has settled long before the loop closes.
        assert!(physical_time(T_CUT - DISSOLVE) > T_COALESCE + 15.0);
    }

    #[test]
    fn exposure_arc_builds_to_the_merger() {
        assert!(exposure(0.0) < exposure(60.0));
        assert!(exposure(60.0) < exposure(T_COALESCE - 0.5));
        assert!(exposure(T_COALESCE + 4.0) < exposure(T_COALESCE - 0.5));
        for t in 0..LOOP_SECONDS as u32 {
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
    fn separation_starts_near_d0_and_coalesces() {
        let opening = separation(0.0);
        assert!((opening - D0).abs() < 2.0, "opening separation {opening}");
        assert!(separation(T_COALESCE - 1e-3) < 2.0);
        assert!(separation(T_COALESCE).abs() < 1e-4);
        assert!(separation(T_COALESCE + 20.0).abs() < 1e-2);
    }

    #[test]
    fn bodies_sit_at_centre_of_mass() {
        for t in [0.0, 30.0, 100.0, 160.0, 173.0] {
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
        let early = orbital_rate(1.0);
        let late = orbital_rate(T_COALESCE - 1.0);
        assert!(late > 8.0 * early, "{early} -> {late}");
        // Never runs back, never jumps: the rate changes by a few percent a
        // frame, most as it whips up to its fastest in the last two seconds.
        let mut prev = orbital_rate(-20.0);
        let mut t = -20.0;
        while t < T_COALESCE {
            t += 1.0 / FPS;
            let rate = orbital_rate(t);
            assert!(rate > 0.0 && orbital_phase(t) > orbital_phase(t - 1.0 / FPS));
            assert!((rate / prev - 1.0).abs() < 0.1, "t={t} {prev} -> {rate}");
            prev = rate;
        }
    }

    #[test]
    fn the_pair_eclipses_on_the_songs_hits() {
        use std::f32::consts::{FRAC_PI_2, PI};
        for &(bar, _) in &ECLIPSES[..ECLIPSES.len() - 1] {
            let t = BAR0 + bar * BAR;
            // A whole number of half turns from the line to the camera...
            let k = (orbital_phase(t) + azimuth(t) - FRAC_PI_2) / PI;
            assert!((k - k.round()).abs() < 1e-3, "bar {bar}: {k} half turns");
            // ...so the holes line up with the camera as it actually stands.
            let f = frame(t);
            let axis = [0, 2].map(|k| f.bodies[0].pos[k] - f.bodies[1].pos[k]);
            let view = [0, 2].map(|k| f.camera.pos[k]);
            let (la, lv) = (axis[0].hypot(axis[1]), view[0].hypot(view[1]));
            let sin = (axis[0] * view[1] - axis[1] * view[0]) / (la * lv);
            assert!(
                sin.abs() < 0.05,
                "bar {bar}: {} deg off",
                sin.asin().to_degrees()
            );
        }
        // Never visibly slows. Where a pass falls on every bar the gaps hold
        // at a bar, give or take a quarter of a percent.
        let mut prev = pass_count(-30.0).1;
        let mut t = -30.0;
        while t < T_COALESCE {
            t += 0.05;
            let rate = pass_count(t).1;
            assert!(rate >= prev * (1.0 - 5e-3), "t={t} {prev} -> {rate}");
            prev = rate;
        }
        // Heavy and slow at the start, about a pass per two bars; an eighth
        // note apart at the end.
        assert!(1.0 / pass_count(10.0).1 > 1.5 * BAR);
        assert!((1.0 / pass_count(T_COALESCE).1 - CLOSEST).abs() < 1e-4);
        // Coalescence falls on the downbeat of bar 54.
        let k = (T_COALESCE - BAR0) / BAR;
        assert!((k - 54.0).abs() < 1e-4);
    }

    #[test]
    fn the_dolly_zoom_holds_the_pair_in_frame() {
        // Through the build the camera pushes in as the lens widens: the pair
        // keeps its size in frame, tan(fov / 2) * distance, while the world
        // round it stretches.
        let framing = |t: f32| {
            let [dist, _, _, fov, _] = camera_key(t);
            (fov.to_radians() / 2.0).tan() * dist
        };
        let start = framing(148.5);
        let mut t = 148.5;
        while t <= T_COALESCE {
            let f = framing(t);
            assert!(
                (f / start - 1.0).abs() < 0.03,
                "t={t} framing {f} vs {start}"
            );
            t += 0.25;
        }
        // It is a real widening, and the lens snaps back after the flash.
        assert!(camera_key(T_COALESCE)[3] > camera_key(148.5)[3] + 15.0);
        assert!(camera_key(T_COALESCE + 1.5)[3] < camera_key(T_COALESCE)[3] - 10.0);
    }

    #[test]
    fn the_tilt_starts_and_lands_without_a_jerk() {
        let rate = |t: f32| (tilt(t + 0.01) - tilt(t - 0.01)) / 0.02;
        // Level on the pair through the film, up at the sky across the seam.
        assert_eq!(tilt(100.0), 0.0);
        assert_eq!(tilt(0.0), TILT_SKY);
        assert!((tilt(T_CUT) - TILT_SKY).abs() < 0.2);
        // Each pan eases out of rest and into it.
        for t in [0.02, 14.48, 201.02, 216.48] {
            assert!(rate(t).abs() < 0.05, "t={t} rate {}", rate(t));
        }
    }

    #[test]
    fn the_sky_drains_into_the_merger_and_floods_back() {
        let (rest, _) = sky_life(100.0);
        let (drained, grey) = sky_life(T_COALESCE - 0.3);
        let (after, colour) = sky_life(T_COALESCE + 0.5);
        assert_eq!(rest, 1.0);
        assert!(drained < 0.5 && grey < 0.5);
        assert!(after > 1.0 && colour > 0.99);
        assert!((sky_life(T_COALESCE + 20.0).0 - 1.0).abs() < 0.01);
        // Continuous through the merger: no pop in the stars.
        assert!((sky_life(T_COALESCE - 1e-4).0 - sky_life(T_COALESCE + 1e-4).0).abs() < 0.01);
    }

    #[test]
    fn the_song_sets_the_loop() {
        // The riser peaks on the downbeat of bar 54, 174.47 s into the track.
        assert!((T_COALESCE - 174.466).abs() < 1e-3);
        // Real time up to the merger, so the passes stay on the beat.
        for t in [0.0, 60.0, 150.0, T_COALESCE] {
            assert_eq!(physical_time(t), t);
        }
    }

    #[test]
    fn rumble_builds_only_toward_the_merger() {
        assert_eq!(rumble(100.0), [0.0; 3]);
        assert_eq!(rumble(T_COALESCE + 0.1), [0.0; 3]);
        let size = |v: [f32; 3]| v.iter().map(|x| x.abs()).fold(0.0, f32::max);
        let peak = |a: f32, b: f32| {
            let mut m = 0.0f32;
            let mut t = a;
            while t < b {
                m = m.max(size(rumble(t)));
                t += 0.01;
            }
            m
        };
        assert!(
            peak(T_COALESCE - 3.0, T_COALESCE) > 3.0 * peak(T_COALESCE - 20.0, T_COALESCE - 17.0)
        );
        // A fraction of the camera's distance: felt, not a lurch.
        assert!(peak(T_COALESCE - 30.0, T_COALESCE) < 0.01);
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
            let jolting = (T_COALESCE..T_COALESCE + 3.5).contains(&frame(t).tau);
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
        assert!(at(T_COALESCE + 0.1)[0] > at(150.0)[0]);
        assert!(at(T_COALESCE + 20.0)[0] < 0.01 * at(T_COALESCE)[0]);
        // Finite differences are finite.
        assert!(table.iter().all(|r| r.iter().all(|v| v.is_finite())));
    }

    #[test]
    fn mini_disks_exist_early_and_vanish_after_merger() {
        assert!(frame(0.0).bodies[0].disk_gain > 0.3);
        assert!(frame(0.0).bodies[0].disk_outer > frame(0.0).bodies[0].disk_inner);
        assert_eq!(frame(T_COALESCE + 8.0).bodies[1].disk_gain, 0.0);
        assert!(frame(T_COALESCE + 8.0).bodies[0].disk_gain > 0.3);
    }

    #[test]
    fn remnant_disk_comes_alive_only_after_the_merger() {
        let merge = loop_time_of(T_COALESCE);
        for t in [0.0, merge - 1.0, merge - 0.01] {
            assert!(frame(t).bodies.iter().all(|b| b.disk_life == 0.0), "t={t}");
        }
        let settled = frame(T_COALESCE + 16.0);
        assert!(settled.bodies[0].disk_life > 0.99);
        assert_eq!(settled.bodies[1].disk_life, 0.0);
    }
}
