// Post pass over linear HDR frames: bloom pyramid, anamorphic streak,
// halation, chromatic aberration, vignette, AgX tonemap, grade, grain, wash.
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
    f: vec4<f32>,       // blur: dir.xy, radius, sigma | downsample: knee
    g: vec4<f32>,       // composite: exposure, wash, bloom, streak
    h: vec4<f32>,       // composite: halation, aberration, vignette, grain
    k: vec4<f32>,       // composite: distortion, saturation, unused, unused
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

fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

fn agx(val: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    var v = m * max(val, vec3<f32>(1e-10));
    v = clamp(log2(v), vec3<f32>(min_ev), vec3<f32>(max_ev));
    v = (v - min_ev) / (max_ev - min_ev);
    return agx_contrast(v);
}

fn agx_eotf(val: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
    );
    let v = m * val;
    return pow(max(v, vec3<f32>(0.0)), vec3<f32>(2.2));
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn grain(p: vec2<u32>, frame: u32) -> f32 {
    // Two-pixel clumps, new every frame, triangular distribution.
    let q = vec2<f32>(p) * 0.5 + vec2<f32>(f32(frame) * 7.31, f32(frame) * 3.17);
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
    let ca = U.h.y * r2;
    let dir = uv - 0.5;
    let base = vec3<f32>(
        bilinear(0u, 0u, U.src_size, 0.5 + dir * (1.0 + ca)).r,
        bilinear(0u, 0u, U.src_size, uv).g,
        bilinear(0u, 0u, U.src_size, 0.5 + dir * (1.0 - ca)).b,
    );

    // Bloom: sum of the pyramid, lower levels weighted more for a wide glow.
    var bloom = vec3<f32>(0.0);
    let weights = array<f32, 6>(0.12, 0.16, 0.2, 0.22, 0.18, 0.12);
    for (var l = 0; l < 6; l++) {
        let lv = U.levels[l];
        bloom += weights[l] * bilinear(1u, lv.z, lv.xy, uv);
    }
    // Halation: red-shifted bleed from the second level.
    let l1 = U.levels[1];
    let hal = bilinear(1u, l1.z, l1.xy, uv) * vec3<f32>(1.0, 0.32, 0.12);
    // Anamorphic streak from the streak buffer, cold blue.
    let sl = U.levels[2];
    let st = bilinear(3u, 0u, sl.xy, uv) * vec3<f32>(0.75, 0.82, 1.08);

    var c = base + bloom * U.g.z + hal * U.h.x + st * U.g.w;

    // Wash: exposure climbs, then the picture melts into warm white.
    let wash = U.g.y;
    c *= U.g.x * exp2(5.5 * wash);

    // Vignette, tighter across the short axis like a real anamorphic.
    let vig = 1.0 - U.h.z * smoothstep(0.15, 1.1, dot(centred * vec2<f32>(0.8, 1.15), centred * vec2<f32>(0.8, 1.15)));
    c *= vig;

    // Grade in scene-linear: teal into the shadows, amber into the highlights.
    let lum = luminance(c);
    let shadow = exp(-lum * 6.0);
    c += vec3<f32>(-0.001, 0.0008, 0.0025) * shadow;
    c *= mix(vec3<f32>(1.0), vec3<f32>(1.06, 1.0, 0.90), smoothstep(0.3, 4.0, lum));

    var t = agx(c);
    // Look: a little punch, saturation.
    let tl = luminance(t);
    t = mix(vec3<f32>(tl), t, U.k.y);
    t = pow(t, vec3<f32>(1.12));
    var lin = agx_eotf(t);
    lin = mix(lin, vec3<f32>(1.0, 0.94, 0.86), smoothstep(0.55, 1.0, wash));

    var out = srgb_encode(clamp(lin, vec3<f32>(0.0), vec3<f32>(1.0)));
    let g = grain(p, U.frame) * U.h.w * (0.25 + 0.75 * (1.0 - luminance(out)));
    out = clamp(out + g, vec3<f32>(0.0), vec3<f32>(1.0));
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
