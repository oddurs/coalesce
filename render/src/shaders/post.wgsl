// Post pass over linear HDR frames: bloom pyramid, anamorphic streak,
// halation, chromatic aberration, vignette, filmic tonemap, grade, grain, wash.
//
// Every pass is one entry point driven by `mode`. Buffers are addressed by id so
// one bind group serves the whole chain: 0 source, 1 mip pyramid, 2 scratch,
// 3 streak, 4 output.

struct Pass {
    src_size: vec2<u32>,
    dst_size: vec2<u32>,
    src_off: u32,
    dst_off: u32,
    src_buf: u32,
    dst_buf: u32,
    mode: u32,
    frame: u32,
    pad0: u32,
    pad1: u32,
    f: vec4<f32>,       // blur: dir.xy, radius, sigma | downsample: knee, threshold
    g: vec4<f32>,       // composite: exposure, unused, bloom, streak
    h: vec4<f32>,       // composite: halation, aberration, vignette, grain
    k: vec4<f32>,       // composite: distortion, saturation, flash, frame scale
    levels: array<vec4<u32>, 6>, // w, h, offset, 0 for each pyramid level
};

@group(0) @binding(0) var<uniform> U: Pass;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> mips: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> scratch: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> streak: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read_write> dst: array<vec4<f32>>;

fn load(buf: u32, off: u32, size: vec2<u32>, p: vec2<i32>) -> vec3<f32> {
    let q = clamp(p, vec2<i32>(0), vec2<i32>(size) - 1);
    let i = off + u32(q.y) * size.x + u32(q.x);
    switch buf {
        case 0u: { return src[i].rgb; }
        case 1u: { return mips[i].rgb; }
        case 2u: { return scratch[i].rgb; }
        case 3u: { return streak[i].rgb; }
        default: { return dst[i].rgb; }
    }
}

fn store(buf: u32, off: u32, size: vec2<u32>, p: vec2<u32>, v: vec3<f32>) {
    let i = off + p.y * size.x + p.x;
    switch buf {
        case 1u: { mips[i] = vec4<f32>(v, 1.0); }
        case 2u: { scratch[i] = vec4<f32>(v, 1.0); }
        case 3u: { streak[i] = vec4<f32>(v, 1.0); }
        default: { dst[i] = vec4<f32>(v, 1.0); }
    }
}

fn bilinear(buf: u32, off: u32, size: vec2<u32>, uv: vec2<f32>) -> vec3<f32> {
    let p = uv * vec2<f32>(size) - 0.5;
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let a = load(buf, off, size, i);
    let b = load(buf, off, size, i + vec2<i32>(1, 0));
    let c = load(buf, off, size, i + vec2<i32>(0, 1));
    let d = load(buf, off, size, i + vec2<i32>(1, 1));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ---------------------------------------------------------------- passes

fn downsample(p: vec2<u32>) {
    let s = vec2<i32>(p) * 2;
    var c = vec3<f32>(0.0);
    // 3x3 tent over the 2x2 block: cheap, stable, no fireflies.
    for (var j = -1; j <= 2; j++) {
        for (var i = -1; i <= 2; i++) {
            let w = select(1.0, 3.0, i >= 0 && i <= 1) * select(1.0, 3.0, j >= 0 && j <= 1);
            c += w * load(U.src_buf, U.src_off, U.src_size, s + vec2<i32>(i, j));
        }
    }
    c /= 64.0;
    if (U.f.x > 0.0) {
        // Soft knee so the dim nebula does not bloom.
        let l = luminance(c);
        let k = smoothstep(0.0, U.f.x, l);
        c *= k;
    }
    if (U.f.y > 0.0) {
        // Hard threshold: only highlights far above the gas feed the streak.
        let l = luminance(c);
        c *= max(l - U.f.y, 0.0) / max(l, 1e-6);
    }
    store(U.dst_buf, U.dst_off, U.dst_size, p, c);
}

fn blur(p: vec2<u32>) {
    let dir = vec2<i32>(i32(U.f.x), i32(U.f.y));
    let radius = i32(U.f.z);
    let sigma = U.f.w;
    var c = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = -radius; i <= radius; i++) {
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        c += w * load(U.src_buf, U.src_off, U.src_size, vec2<i32>(p) + dir * i);
        wsum += w;
    }
    store(U.dst_buf, U.dst_off, U.dst_size, p, c / wsum);
}

// ---------------------------------------------------------------- tonemap

// Per-channel filmic curve (Narkowicz's ACES fit). Channels clip in turn, so
// overexposed amber climbs through yellow to white the way fire and film do.
// A hue-preserving curve held the disks flat and grey.
fn filmic(x: vec3<f32>) -> vec3<f32> {
    let y = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return clamp(y, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn grain(p: vec2<u32>, frame: u32, clump: f32) -> f32 {
    // Clumps of a fixed fraction of the frame, new every frame.
    let q = vec2<f32>(p) / clump + vec2<f32>(f32(frame) * 7.31, f32(frame) * 3.17);
    let i = vec2<i32>(floor(q));
    let f = fract(q);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i, frame);
    let b = hash2(i + vec2<i32>(1, 0), frame);
    let c = hash2(i + vec2<i32>(0, 1), frame);
    let d = hash2(i + vec2<i32>(1, 1), frame);
    let n = mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
    let n2 = hash2(vec2<i32>(p), frame + 977u);
    return (n - 0.5) * 1.4 + (n2 - 0.5) * 0.4;
}

fn hash2(p: vec2<i32>, salt: u32) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ salt * 83492791u;
    h = h * 747796405u + 2891336453u;
    let w = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return f32((w >> 22u) ^ w) / 4294967296.0;
}

fn composite(p: vec2<u32>) {
    let size = vec2<f32>(U.dst_size);
    let uv0 = (vec2<f32>(p) + 0.5) / size;
    let aspect = size.x / size.y;
    let centred = (uv0 - 0.5) * vec2<f32>(aspect, 1.0);
    let r2 = dot(centred, centred);

    // Mild barrel distortion, then radial chromatic aberration.
    let r2_corner = 0.25 * aspect * aspect + 0.25;
    let distort = (1.0 + U.k.x * r2) / (1.0 + U.k.x * r2_corner);
    let uv = 0.5 + (uv0 - 0.5) * distort;
    // Lateral aberration as a spectrum: seven taps from the inner to the outer
    // offset, each channel weighted over the part of the spread its colour
    // falls in. Three hard channel offsets split corner stars into separate
    // red, green and blue copies.
    let ca = U.h.y * r2;
    let dir = uv - 0.5;
    var base = vec3<f32>(0.0);
    var wsum = vec3<f32>(0.0);
    for (var k = 0; k < 7; k++) {
        let s = f32(k) / 3.0 - 1.0;
        let w = max(vec3<f32>(0.0), 1.0 - abs(vec3<f32>(s) - vec3<f32>(0.66, 0.0, -0.66)) / 0.9);
        base += w * bilinear(0u, 0u, U.src_size, 0.5 + dir * (1.0 + ca * s));
        wsum += w;
    }
    base /= wsum;

    // Lens falloff: the glass softens a little toward the corners, the way
    // real (and especially anamorphic) lenses do, keeping the eye centred.
    let soft = smoothstep(0.3, 1.15, r2) * 0.55;
    if (soft > 0.0) {
        let o = 1.2 * U.k.w / vec2<f32>(U.src_size);
        var blur = vec3<f32>(0.0);
        for (var j = 0; j < 4; j++) {
            let a = f32(j) * 1.5708 + 0.7854;
            blur += bilinear(0u, 0u, U.src_size, uv + vec2<f32>(cos(a), sin(a)) * o);
        }
        base = mix(base, blur * 0.25, soft);
    }

    // Bloom: sum of the pyramid, weighted to the finer levels. Glare hugs the
    // hot gas; weighting the widest levels laid a veil over the shadows.
    var bloom = vec3<f32>(0.0);
    let weights = array<f32, 6>(0.24, 0.26, 0.22, 0.14, 0.09, 0.05);
    for (var l = 0; l < 6; l++) {
        let lv = U.levels[l];
        bloom += weights[l] * bilinear(1u, lv.z, lv.xy, uv);
    }
    // Halation: red-shifted bleed from the second level.
    let l1 = U.levels[1];
    let hal = bilinear(1u, l1.z, l1.xy, uv) * vec3<f32>(1.0, 0.32, 0.12);
    // Anamorphic streak from the streak buffer, cold blue.
    let sl = U.levels[0];
    let st = bilinear(3u, 0u, sl.xy, uv) * vec3<f32>(0.75, 0.82, 1.08);

    var c = base + bloom * U.g.z + hal * U.h.x + st * U.g.w;

    // Flash: the merger kicks the exposure for a few frames.
    let flash = U.k.z;
    c *= U.g.x * (1.0 + 0.8 * flash);

    // Vignette, tighter across the short axis like a real anamorphic.
    let vig = 1.0 - U.h.z * smoothstep(0.15, 1.1, dot(centred * vec2<f32>(0.8, 1.15), centred * vec2<f32>(0.8, 1.15)));
    c *= vig;

    // Grade in scene-linear, a split tone: dim gas leans to embers, the hot
    // core stays cream. Multiplicative, so space stays black; a lifted teal
    // shadow here turned the dim disk brown. Weighted by how warm the pixel
    // already is, so white and blue stars stay a cool counterpoint.
    let lum = luminance(c);
    let warmth = smoothstep(0.05, 0.4, (c.r - c.b) / (c.r + c.b + 1e-4));
    let tone = mix(vec3<f32>(1.14, 0.84, 0.62), vec3<f32>(1.04, 0.99, 0.92), smoothstep(0.02, 1.5, lum));
    c *= mix(vec3<f32>(1.0), tone, warmth);

    // Saturation in scene-linear, before the curve bends it.
    let cl = luminance(c);
    c = max(mix(vec3<f32>(cl), c, U.k.y), vec3<f32>(0.0));
    let lin = filmic(c);

    var out = srgb_encode(clamp(lin, vec3<f32>(0.0), vec3<f32>(1.0)));
    // Film grain lives in the midtones: a print's dense blacks and its
    // clipped highlights hold almost none, so empty space stays clean rather
    // than fizzing with digital noise. A little of it is colour grain.
    let ol = luminance(out);
    let amount = U.h.w * (0.15 + 3.4 * ol * (1.0 - ol));
    let clump = 1.5 * U.k.w;
    let mono = grain(p, U.frame, clump);
    let chroma = vec3<f32>(
        grain(p, U.frame + 101u, clump),
        grain(p, U.frame + 211u, clump),
        grain(p, U.frame + 307u, clump),
    );
    out = clamp(out + amount * (0.8 * mono + 0.35 * chroma), vec3<f32>(0.0), vec3<f32>(1.0));
    store(4u, 0u, U.dst_size, p, out);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let p = gid.xy;
    if (p.x >= U.dst_size.x || p.y >= U.dst_size.y) { return; }
    switch U.mode {
        case 0u: { downsample(p); }
        case 1u: { blur(p); }
        default: { composite(p); }
    }
}
