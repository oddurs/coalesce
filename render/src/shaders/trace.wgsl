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
    bodies: array<Body, 2>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read_write> accum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> blackbody: array<vec4<f32>>;

const PI: f32 = 3.14159265;
const ESCAPE_RADIUS: f32 = 90.0;
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
    inner: f32, outer: f32, gain: f32, temp_in: f32, seed: f32, thick: f32, scale: f32,
) -> DiskSample {
    var out: DiskSample;
    out.emission = vec3<f32>(0.0);
    out.density = 0.0;
    if (gain <= 0.0) { return out; }
    let rel = x - center;
    let z = dot(rel, normal);
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
    var dens = pow(smoothstep(0.40, 0.72, n), 1.2);
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
    out.emission = col * brightness * dens * (0.4 + 1.2 * dens) * gain;
    out.density = dens * min(gain, 1.0);
    return out;
}

fn sample_volume(x: vec3<f32>, dir: vec3<f32>) -> DiskSample {
    var total: DiskSample;
    total.emission = vec3<f32>(0.0);
    total.density = 0.0;
    for (var i = 0; i < 2; i++) {
        let b = P.bodies[i];
        let s = disk_sample(
            x, dir, b.pos_rs.xyz, b.vel_mass.xyz, b.normal.xyz, b.tangent.xyz,
            b.vel_mass.w, b.pos_rs.w, b.disk.x, b.disk.y, b.disk.z, b.disk.w,
            f32(i) * 7.3, 0.06, 1.0,
        );
        total.emission += s.emission;
        total.density += s.density;
    }
    // Circumbinary disk around the centre of mass.
    let big = disk_sample(
        x, dir, vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
        1.0, 2.0, P.big.x, P.big.y, P.big.z, P.big.w, 31.7, 0.012, 2.5,
    );
    total.emission += big.emission;
    total.density += big.density;
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
            let delta = uv - pos;
            let dist2 = dot(delta, delta) / (size * size);
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
    col += star_layer(d, 900.0, 0.012, 11u, px_size * 900.0 * 0.55) * 0.8;
    col += star_layer(d, 2400.0, 0.012, 23u, px_size * 2400.0 * 0.45) * 0.06;
    col += star_layer(d, 300.0, 0.006, 37u, px_size * 300.0 * 0.7) * 3.0;
    col *= P.sky.x;

    // Nebula: faint warm dust and a cold gas band, well below the disks.
    let w = fbm(d * 2.1 + vec3<f32>(3.0), 4);
    let band = exp(-pow(d.y + 0.15 * w - 0.05, 2.0) * 22.0);
    let dust = fbm(d * 4.0 + vec3<f32>(w * 1.5, 0.0, 9.0), 5);
    let gas = fbm(d * 3.0 + vec3<f32>(0.0, w * 2.0, 4.0), 5);
    let wisps = fbm(d * 9.0 + vec3<f32>(gas * 3.0, dust * 2.0, 1.0), 4);
    let warm = vec3<f32>(1.0, 0.5, 0.22) * pow(dust, 6.0) * 1.2 * (0.5 + wisps);
    let cold = vec3<f32>(0.2, 0.5, 1.0) * pow(gas, 7.0) * 0.6 * (0.4 + wisps);
    col += ((warm + cold) * band + vec3<f32>(0.004, 0.006, 0.012) * pow(gas, 3.0)) * P.sky.y;
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
        h = min(h, disk_step_limit(x, b.pos_rs.xyz, b.normal.xyz, b.disk.x, b.disk.y, b.disk.z, 0.06, 1.0));
    }
    h = min(h, disk_step_limit(x, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), P.big.x, P.big.y, P.big.z, 0.012, 2.5));
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
        var h = clamp(0.09 * rmin, 0.03, 0.6);
        h = min(h, step_limit(x));
        h /= length(v);

        // RK4 for the second order system x'' = accel(x, x').
        let k1v = accel(x, v);
        let k1x = v;
        let k2v = accel(x + 0.5 * h * k1x, v + 0.5 * h * k1v);
        let k2x = v + 0.5 * h * k1v;
        let k3v = accel(x + 0.5 * h * k2x, v + 0.5 * h * k2v);
        let k3x = v + 0.5 * h * k2v;
        let k4v = accel(x + h * k3x, v + h * k3v);
        let k4x = v + h * k3v;
        let dx = (h / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        let dv = (h / 6.0) * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        let mid = x + 0.5 * dx;
        let seg = length(dx);
        x += dx;
        v += dv;

        let s = sample_volume(mid, v);
        if (s.density > 0.0) {
            col += transmit * s.emission * seg;
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

    // Thin lens with an anamorphic aperture: tall oval bokeh.
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
