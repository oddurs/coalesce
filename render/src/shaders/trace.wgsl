// Geodesic ray tracer for a binary black hole.
//
// Light paths are integrated with the Schwarzschild orbit equation in vector
// form, a = -1.5 rs h^2 x / r^5 with h = |x cross v|, which reproduces the exact
// null geodesic shape for one body. The two bodies are superposed. Emission is
// accumulated volumetrically from three accretion disks with Doppler and
// gravitational shifts applied to a blackbody colour lookup.

struct Body {
    pos_rs: vec4<f32>,      // xyz position, w Schwarzschild radius
    vel_mass: vec4<f32>,    // xyz velocity (c units), w mass
    disk: vec4<f32>,        // inner, outer, gain, temperature
    normal: vec4<f32>,      // disk normal
    tangent: vec4<f32>,     // disk in-plane axis
};

struct Params {
    cam_pos: vec4<f32>,     // xyz, w focus distance
    cam_u: vec4<f32>,       // xyz right, w tan(half fov x)
    cam_v: vec4<f32>,       // xyz up, w tan(half fov y)
    cam_w: vec4<f32>,       // xyz forward, w aperture radius
    res: vec4<u32>,         // width, height, tile x, tile y
    misc: vec4<f32>,        // time, sample index, max steps, unused
    big: vec4<f32>,         // circumbinary inner, outer, gain, temperature
    sky: vec4<f32>,         // star gain, nebula gain, star size, unused
    gw: vec4<f32>,          // wave speed, table t0, table dt, table length
    gw2: vec4<f32>,         // lens strength, disk ripple strength, flash, stream gain
    spiral: vec4<f32>,      // spiral phase, strength, lump phase, lump strength
    bodies: array<Body, 2>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read_write> accum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> blackbody: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> gw_table: array<vec4<f32>>;

const PI: f32 = 3.14159265;
const ESCAPE_RADIUS: f32 = 120.0;
const ABSORB: f32 = 0.9;

// ---------------------------------------------------------------- random

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

var<private> rng_state: u32;

fn rand() -> f32 {
    rng_state = pcg(rng_state);
    return f32(rng_state) / 4294967296.0;
}

fn hash3(p: vec3<i32>) -> f32 {
    let h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    return f32(pcg(h)) / 4294967296.0;
}

fn hash3v(p: vec3<i32>, salt: u32) -> vec3<f32> {
    let h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u ^ salt;
    let a = pcg(h);
    let b = pcg(a);
    let c = pcg(b);
    return vec3<f32>(f32(a), f32(b), f32(c)) / 4294967296.0;
}

// ---------------------------------------------------------------- noise

fn noise3(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash3(i);
    let b = hash3(i + vec3<i32>(1, 0, 0));
    let c = hash3(i + vec3<i32>(0, 1, 0));
    let d = hash3(i + vec3<i32>(1, 1, 0));
    let e = hash3(i + vec3<i32>(0, 0, 1));
    let g = hash3(i + vec3<i32>(1, 0, 1));
    let h = hash3(i + vec3<i32>(0, 1, 1));
    let k = hash3(i + vec3<i32>(1, 1, 1));
    let x0 = mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
    let x1 = mix(mix(e, g, u.x), mix(h, k, u.x), u.y);
    return mix(x0, x1, u.z);
}

fn fbm(p_in: vec3<f32>, octaves: i32) -> f32 {
    var p = p_in;
    var a = 0.5;
    var s = 0.0;
    var n = 0.0;
    for (var i = 0; i < octaves; i++) {
        s += a * noise3(p);
        n += a;
        a *= 0.5;
        p = p * 2.03 + vec3<f32>(17.1, 9.3, 3.7);
    }
    return s / n;
}

// ---------------------------------------------------------------- colour

fn bb_color(temp: f32) -> vec3<f32> {
    let t = clamp(log(temp / 1000.0) / log(30.0), 0.0, 1.0) * 255.0;
    let i = u32(t);
    let f = t - f32(i);
    let a = blackbody[min(i, 255u)].rgb;
    let b = blackbody[min(i + 1u, 255u)].rgb;
    return mix(a, b, f);
}

// ---------------------------------------------------------------- waves

// Strain table at emission time: amplitude, phase, and their time derivatives.
fn gw_lookup(t: f32) -> vec4<f32> {
    let f = clamp((t - P.gw.y) / P.gw.z, 0.0, P.gw.w - 1.001);
    let i = u32(f);
    let w = f - f32(i);
    return mix(gw_table[i], gw_table[i + 1u], w);
}

struct Strain {
    h: f32,
    grad: vec3<f32>,
};

// Quadrupole wave radiated from the centre of mass, evaluated at retarded
// time so ripples sweep outward at the wave speed. Analytic gradient.
fn gw_strain(x: vec3<f32>) -> Strain {
    var out: Strain;
    let r = max(length(x), 3.0);
    let rho = max(length(x.xz), 0.5);
    let t_ret = P.misc.x - r / P.gw.x;
    let s = gw_lookup(t_ret);
    let phi = atan2(x.z, x.x);
    let arg = 2.0 * phi - s.y;
    let c = cos(arg);
    let sn = sin(arg);
    out.h = s.x * c / r;
    // d/dr through the retarded time and the 1/r falloff.
    let dh_dr = (-(s.z * c + s.x * sn * s.w) / P.gw.x) / r - out.h / r;
    let dh_dphi = -2.0 * s.x * sn / r;
    let rhat = x / r;
    let phihat = vec3<f32>(-x.z, 0.0, x.x) / rho;
    out.grad = dh_dr * rhat + (dh_dphi / rho) * phihat;
    return out;
}

// ---------------------------------------------------------------- gravity

fn accel(x: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    var a = vec3<f32>(0.0);
    for (var i = 0; i < 2; i++) {
        let r = x - P.bodies[i].pos_rs.xyz;
        let rs = P.bodies[i].pos_rs.w;
        let r2 = dot(r, r);
        let h = cross(r, v);
        let h2 = dot(h, h);
        let r5 = r2 * r2 * sqrt(r2);
        a -= 1.5 * rs * h2 * r / max(r5, 1e-6);
    }
    return a;
}

// ---------------------------------------------------------------- disks

struct DiskSample {
    emission: vec3<f32>,
    density: f32,
};

fn doppler(gas_vel: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    let b2 = dot(gas_vel, gas_vel);
    let gamma = 1.0 / sqrt(max(1.0 - b2, 1e-4));
    // Photons travel toward the camera, opposite to the marching direction.
    let toward = -normalize(ray_dir);
    return 1.0 / (gamma * (1.0 - dot(gas_vel, toward)));
}

// Volumetric disk around `center` with the given basis. Returns emission and
// extinction density at point x.
fn disk_sample(
    x: vec3<f32>, ray_dir: vec3<f32>, center: vec3<f32>, center_vel: vec3<f32>,
    normal: vec3<f32>, tangent: vec3<f32>, mass: f32, rs_grav: f32,
    inner: f32, outer: f32, gain: f32, temp_in: f32, seed: f32, thick: f32, scale: f32, ripple: f32, driven: f32, falloff: f32,
) -> DiskSample {
    var out: DiskSample;
    out.emission = vec3<f32>(0.0);
    out.density = 0.0;
    if (gain <= 0.0) { return out; }
    let rel = x - center;
    let z = dot(rel, normal) - ripple;
    let inplane = rel - z * normal;
    let r = length(inplane);
    if (r < inner * 0.85 || r > outer * 1.15) { return out; }
    let thickness = thick * r + 0.01 * scale;
    let zn = z / thickness;
    if (abs(zn) > 3.0) { return out; }

    let bitangent = cross(normal, tangent);
    let phi = atan2(dot(inplane, bitangent), dot(inplane, tangent));
    // Keplerian shear, scaled like the orbit so the disks rotate on screen.
    let omega = 22.0 * pow(max(r, 0.5), -1.5) * sqrt(mass);
    let phi_rot = phi - omega * P.misc.x * 0.35;
    // Features are stretched along the orbit: coarse in phi, periodic in phi by
    // construction, and laid out on a normalised log radius so the number of
    // radial cycles is fixed per disk regardless of its size.
    let rho = log(max(r, inner * 0.5) / inner) / log(outer / inner);
    let az = 2.4;
    let cycles = 2.5 * scale;
    let q = vec3<f32>(cos(phi_rot) * az, sin(phi_rot) * az, rho * cycles + seed);
    let warp = fbm(q + vec3<f32>(seed, 0.0, zn * 0.4), 3);
    let n1 = fbm(q * 2.0 + vec3<f32>(warp * 1.6, warp * 1.1, zn * 0.6), 4);
    let rings = fbm(vec3<f32>(rho * cycles * 2.2, seed * 1.7, 0.0), 3);
    let fine = fbm(q * 3.0 + vec3<f32>(seed, warp * 2.0, zn), 3);
    let n = n1 * 0.55 + rings * 0.3 + fine * 0.15;
    let n2 = fine;
    // A continuous body of gas with turbulence carved into it. A hard threshold
    // here leaves isolated ribbons that read as wire, not plasma.
    let clump = smoothstep(0.3, 0.75, n);
    var dens = 0.22 + 0.78 * clump * clump;
    if (driven > 0.0) {
        // Two-armed trailing spiral locked to the binary, and the lopsided
        // overdensity that orbits the cavity edge. Both fade outward.
        let wind = 3.0 * log(max(r, inner) / inner);
        let arms = cos(2.0 * (phi - P.spiral.x) + 2.0 * wind);
        let arm_fade = exp(-(r - inner) / (0.6 * (outer - inner)));
        let lump = cos(phi - P.spiral.z);
        let lump_fade = exp(-(r - inner) / (0.25 * inner));
        dens *= (1.0 + P.spiral.y * driven * arms * arm_fade) * (1.0 + P.spiral.w * driven * lump * lump_fade);
    }
    let edge_in = smoothstep(inner * 0.85, inner * 1.05, r);
    let edge_out = 1.0 - smoothstep(outer * 0.8, outer * 1.15, r);
    let vertical = exp(-zn * zn);
    let radial = pow(inner / r, 1.6);
    dens *= edge_in * edge_out * vertical * radial;

    let vmag = sqrt(mass / max(r, 0.5));
    let gas_vel = center_vel + vmag * normalize(cross(normal, inplane));
    let g_dop = doppler(gas_vel, ray_dir);
    let g_grav = sqrt(max(1.0 - rs_grav / max(r, rs_grav * 1.01), 0.02));
    let g = g_dop * g_grav;
    let temp = temp_in * pow(inner / r, 0.75) * (0.85 + 0.3 * n2);
    let col = bb_color(temp * g);
    let brightness = pow(g, 3.0) * pow(temp / 6500.0, 2.4);
    // Extra emissive falloff, separate from the gas: the outskirts burn down to
    // embers and the hot inner edge carries the frame.
    let glow = pow(inner / r, falloff);
    out.emission = col * brightness * glow * dens * (0.4 + 1.2 * dens) * gain;
    out.density = dens * min(gain, 1.0);
    return out;
}

// Accretion stream: gas torn from the cavity edge spirals in to the hole,
// trailing behind it. Distance to a short polyline, Gaussian falloff.
fn stream_density(x: vec3<f32>, hole: vec3<f32>, mini_outer: f32, cavity: f32, seed: f32) -> f32 {
    if (abs(x.y) > 2.5) { return 0.0; }
    let rho_h = length(hole.xz);
    let rho_x = length(x.xz);
    if (rho_x < rho_h * 0.8 || rho_x > cavity * 1.05) { return 0.0; }
    let phi_h = atan2(hole.z, hole.x);
    let lag = 1.4;
    var best = 1e9;
    var best_s = 0.0;
    var prev = hole;
    for (var i = 1; i <= 12; i++) {
        let sg = f32(i) / 12.0;
        let ang = phi_h - lag * sg * sg;
        let rad = rho_h + (cavity * 0.98 - rho_h) * sg;
        let pnt = vec3<f32>(cos(ang) * rad, 0.0, sin(ang) * rad);
        // Distance to the segment, so the stream is a continuous ribbon.
        let seg = pnt - prev;
        let u = clamp(dot(x - prev, seg) / max(dot(seg, seg), 1e-4), 0.0, 1.0);
        let d = length(x - (prev + seg * u));
        if (d < best) { best = d; best_s = (f32(i - 1) + u) / 12.0; }
        prev = pnt;
    }
    let width = 0.15 + 0.35 * best_s;
    let along = fbm(x * 1.6 + vec3<f32>(seed, P.misc.x * 0.5, 0.0), 3);
    let taper = smoothstep(0.0, 0.08, best_s) * (1.0 - smoothstep(0.85, 1.0, best_s));
    // The stream feeds the mini-disk's rim and hands over to it there. Carried
    // on to the hole, it met the horizon at full strength and rays that ended
    // there cut it off in hard-edged boxes.
    let feed = smoothstep(0.7 * mini_outer, 1.1 * mini_outer, length(x - hole));
    return exp(-(best * best) / (width * width)) * (0.6 + 0.4 * along) * taper * feed;
}

// Soft corona above a disk: scattered light in a smooth layer hugging it.
// Gives the disks depth without the cost of more turbulence.
fn corona(x: vec3<f32>, center: vec3<f32>, normal: vec3<f32>, inner: f32, outer: f32, gain: f32, temp: f32) -> vec3<f32> {
    if (gain <= 0.0) { return vec3<f32>(0.0); }
    let rel = x - center;
    let z = abs(dot(rel, normal));
    let r = length(rel - dot(rel, normal) * normal);
    if (r < inner * 0.7 || r > outer * 1.3) { return vec3<f32>(0.0); }
    // Gaussian, so it has faded to nothing long before the march cares where
    // it ends. An exponential this tall was clipped mid-glow at the disk slab.
    let height = 0.025 * r + 0.03;
    let vertical = exp(-(z * z) / (height * height));
    let radial = pow(inner / max(r, inner), 2.2) * smoothstep(inner * 0.7, inner * 1.1, r) * (1.0 - smoothstep(outer * 0.9, outer * 1.3, r));
    return bb_color(temp * 0.9) * vertical * radial * gain * 0.008;
}

fn sample_volume(x: vec3<f32>, dir: vec3<f32>) -> DiskSample {
    var total: DiskSample;
    total.emission = vec3<f32>(0.0);
    total.density = 0.0;
    let wave = gw_strain(x);
    for (var i = 0; i < 2; i++) {
        let b = P.bodies[i];
        let s = disk_sample(
            x, dir, b.pos_rs.xyz, b.vel_mass.xyz, b.normal.xyz, b.tangent.xyz,
            b.vel_mass.w, b.pos_rs.w, b.disk.x, b.disk.y, b.disk.z, b.disk.w,
            f32(i) * 7.3, 0.04, 1.0, 0.0, 0.0, 0.0,
        );
        total.emission += s.emission;
        total.density += s.density;
        total.emission += corona(x, b.pos_rs.xyz, b.normal.xyz, b.disk.x, b.disk.y, b.disk.z, b.disk.w);
        if (b.disk.z > 0.0 && P.gw2.w > 0.0) {
            let st = stream_density(x, b.pos_rs.xyz, b.disk.y, P.big.x, f32(i) * 3.1) * P.gw2.w * b.disk.z;
            total.emission += bb_color(4600.0) * st * 0.15;
            total.density += st * 0.15;
        }
    }
    // Circumbinary disk around the centre of mass, rippled by the waves.
    let ripple = P.gw2.y * wave.h * length(x.xz);
    let big = disk_sample(
        x, dir, vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
        1.0, 2.0, P.big.x, P.big.y, P.big.z, P.big.w, 31.7, 0.009, 2.5, ripple, 1.0, 1.6,
    );
    total.emission += big.emission;
    total.density += big.density;
    total.emission += corona(x, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), P.big.x, P.big.y, P.big.z, P.big.w);
    // Merger flash: the shock lights the gas near the core in the first
    // moments. Weighted by density so empty space stays dark.
    let flash = P.gw2.z;
    if (flash > 0.0) {
        total.emission += bb_color(7500.0) * exp(-length(x) / 3.5) * 2.0 * flash * total.density;
    }
    return total;
}

// ---------------------------------------------------------------- sky

fn octa_encode(d: vec3<f32>) -> vec2<f32> {
    let n = d / (abs(d.x) + abs(d.y) + abs(d.z));
    var uv = n.xz;
    if (n.y < 0.0) {
        let sgn = vec2<f32>(select(-1.0, 1.0, uv.x >= 0.0), select(-1.0, 1.0, uv.y >= 0.0));
        uv = (1.0 - abs(uv.yx)) * sgn;
    }
    return uv * 0.5 + 0.5;
}

fn octa_decode(uv_in: vec2<f32>) -> vec3<f32> {
    let f = uv_in * 2.0 - 1.0;
    var n = vec3<f32>(f.x, 1.0 - abs(f.x) - abs(f.y), f.y);
    let t = clamp(-n.y, 0.0, 1.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.z += select(t, -t, n.z >= 0.0);
    return normalize(n);
}

// `size` is the star's angular sigma in radians; the shape is measured on the
// sphere so stars stay round everywhere on the octahedral map.
fn star_layer(d: vec3<f32>, cells: f32, density: f32, salt: u32, size: f32) -> vec3<f32> {
    let uv = octa_encode(d) * cells;
    let base = vec2<i32>(floor(uv));
    var col = vec3<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = base + vec2<i32>(i, j);
            let h = hash3v(vec3<i32>(c.x, c.y, 0), salt);
            if (h.x > density) { continue; }
            let pos = vec2<f32>(c) + hash3v(vec3<i32>(c.x, c.y, 1), salt).xy;
            let star_dir = octa_decode(pos / cells);
            let cosang = clamp(dot(d, star_dir), -1.0, 1.0);
            let ang = sqrt(max(2.0 * (1.0 - cosang), 0.0));
            let dist2 = ang * ang / (size * size);
            let mag = pow(h.y, 9.0) * 24.0 + 0.02;
            let temp = 2500.0 + 12000.0 * h.z * h.z;
            col += bb_color(temp) * mag * exp(-dist2);
        }
    }
    return col;
}

fn sky(d: vec3<f32>) -> vec3<f32> {
    let px_size = P.sky.z;
    var col = vec3<f32>(0.0);
    col += star_layer(d, 900.0, 0.012, 11u, px_size * 1.0) * 0.8;
    col += star_layer(d, 2400.0, 0.016, 23u, px_size * 0.8) * 0.05;
    col += star_layer(d, 300.0, 0.006, 37u, px_size * 1.3) * 3.0;
    // A handful of bright stars with a soft halo: these lens into arcs.
    col += star_layer(d, 120.0, 0.004, 53u, px_size * 1.2) * 14.0;
    col += star_layer(d, 120.0, 0.004, 53u, px_size * 5.0) * 0.6;
    col *= P.sky.x;

    // Nebula: faint warm dust and a cold gas band, well below the disks.
    let w = fbm(d * 2.1 + vec3<f32>(3.0), 4);
    let band = exp(-pow(d.y + 0.15 * w - 0.05, 2.0) * 22.0);
    let dust = fbm(d * 4.0 + vec3<f32>(w * 1.5, 0.0, 9.0), 5);
    let gas = fbm(d * 3.0 + vec3<f32>(0.0, w * 2.0, 4.0), 5);
    let wisps = fbm(d * 9.0 + vec3<f32>(gas * 3.0, dust * 2.0, 1.0), 4);
    let warm = vec3<f32>(1.0, 0.42, 0.18) * pow(dust, 7.0) * 1.3 * (0.4 + wisps);
    let cold = vec3<f32>(0.2, 0.5, 1.0) * pow(gas, 7.0) * 0.6 * (0.4 + wisps);
    col += ((warm + cold) * band * 0.9 + vec3<f32>(0.003, 0.005, 0.010) * pow(gas, 3.0)) * P.sky.y;
    return col;
}

// Largest step that will not skip across a disk slab. Inside the slab the
// step is a fraction of the thickness; outside it is half the distance to it.
fn disk_step_limit(x: vec3<f32>, center: vec3<f32>, normal: vec3<f32>, inner: f32, outer: f32, gain: f32, thick: f32, scale: f32) -> f32 {
    if (gain <= 0.0) { return 1e9; }
    let rel = x - center;
    let z = abs(dot(rel, normal));
    let r = length(rel - dot(rel, normal) * normal);
    if (r < inner * 0.7 || r > outer * 1.3) { return 1e9; }
    let thickness = thick * r + 0.01 * scale;
    let slab = 3.0 * thickness;
    if (z < slab) { return max(0.3 * thickness, 0.015); }
    return max(0.5 * (z - slab), 0.05);
}

fn step_limit(x: vec3<f32>) -> f32 {
    var h = 1e9;
    for (var i = 0; i < 2; i++) {
        let b = P.bodies[i];
        h = min(h, disk_step_limit(x, b.pos_rs.xyz, b.normal.xyz, b.disk.x, b.disk.y, b.disk.z, 0.04, 1.0));
    }
    h = min(h, disk_step_limit(x, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), P.big.x, P.big.y, P.big.z, 0.009, 2.5));
    return h;
}

// ---------------------------------------------------------------- trace

fn trace(origin: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    var x = origin;
    var v = dir;
    var col = vec3<f32>(0.0);
    var transmit = 1.0;
    let max_steps = i32(P.misc.z);

    for (var step = 0; step < max_steps; step++) {
        var rmin = 1e9;
        for (var i = 0; i < 2; i++) {
            let r = length(x - P.bodies[i].pos_rs.xyz);
            if (r < P.bodies[i].pos_rs.w) { return col; }
            rmin = min(rmin, r);
        }
        let rc = length(x);
        if (rc > ESCAPE_RADIUS && dot(x, v) > 0.0) {
            return col + transmit * sky(normalize(v));
        }

        // Step control: fine near the horizons and inside the disk slab.
        // Coarse far out, where the field is weak and the camera may sit.
        let far = 0.6 + 0.04 * max(rc - 60.0, 0.0);
        var h = clamp(0.09 * rmin, 0.03, far);
        h = min(h, step_limit(x));
        h /= length(v);

        // Wave lensing: a weak refractive gradient, held across the step.
        let a_gw = P.gw2.x * gw_strain(x).grad;

        // RK4 for the second order system x'' = accel(x, x').
        let k1v = accel(x, v) + a_gw;
        let k1x = v;
        let k2v = accel(x + 0.5 * h * k1x, v + 0.5 * h * k1v) + a_gw;
        let k2x = v + 0.5 * h * k1v;
        let k3v = accel(x + 0.5 * h * k2x, v + 0.5 * h * k2v) + a_gw;
        let k3x = v + 0.5 * h * k2v;
        let k4v = accel(x + h * k3x, v + h * k3v) + a_gw;
        let k4x = v + h * k3v;
        let dx = (h / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        let dv = (h / 6.0) * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        // Sample the volume at a random point along the step, not its middle:
        // neighbouring pixels take near-identical steps, and fixed sample
        // points lined up into contour ripples across grazing gas.
        let mid = x + rand() * dx;
        let seg = length(dx);
        x += dx;
        v += dv;

        let s = sample_volume(mid, v);
        // Corona and flash glow where there is no gas to absorb. Gating the
        // emission on density clipped them to the disk slabs, and lensing drew
        // those clips as hard-edged boxes.
        col += transmit * s.emission * seg;
        if (s.density > 0.0) {
            transmit *= exp(-ABSORB * s.density * seg);
            if (transmit < 0.005) { return col; }
        }
    }
    return col;
}

// ---------------------------------------------------------------- entry

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.xy + P.res.zw;
    if (px.x >= P.res.x || px.y >= P.res.y) { return; }
    let sample = u32(P.misc.y);
    rng_state = pcg(px.x * 1973u + px.y * 9277u + sample * 26699u + 1u);

    let jitter = vec2<f32>(rand(), rand());
    let uv = (vec2<f32>(px) + jitter) / vec2<f32>(P.res.xy);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let fwd = P.cam_w.xyz;
    let dir_pin = normalize(fwd + P.cam_u.xyz * ndc.x * P.cam_u.w + P.cam_v.xyz * ndc.y * P.cam_v.w);

    // Thin lens with an anamorphic aperture: tall oval bokeh. The production
    // camera is a pinhole; bokeh needs far more samples than the disks do.
    let focus = P.cam_pos.xyz + dir_pin * (P.cam_pos.w / dot(dir_pin, fwd));
    let ang = rand() * 2.0 * PI;
    let rad = sqrt(rand()) * P.cam_w.w;
    let ap = vec2<f32>(cos(ang) * 0.55, sin(ang)) * rad;
    let origin = P.cam_pos.xyz + P.cam_u.xyz * ap.x + P.cam_v.xyz * ap.y;
    let dir = normalize(focus - origin);

    let c = trace(origin, dir);
    let idx = px.y * P.res.x + px.x;
    accum[idx] += vec4<f32>(c, 1.0);
}
