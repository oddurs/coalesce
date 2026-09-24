# black-hole

A binary black hole inspiral, merger and ringdown, rendered as a seamless 60 second
loop at 4096x2160 in the 1.90:1 IMAX digital frame, and a SvelteKit site that plays it.

The renderer is a custom GPU ray tracer. Production renderers trace straight rays;
a black hole bends them, so every pixel here integrates a curved light path through the
combined gravitational field of both bodies. A second pass grades the linear frames with
bloom, halation, anamorphic streaks, grain and a filmic tonemap.

## Requirements

Rust (see `rust-toolchain.toml`), pnpm, ffmpeg, and a GPU with Metal or Vulkan.

## Quickstart

```sh
scripts/setup
scripts/task check
scripts/render --preview --t 12   # one 1080p frame to out/preview/still.png
scripts/render --lo               # the whole loop at 960x506, a few minutes
scripts/render                    # all 1440 frames at 4K, resumable, about 6 h on an M4 Pro
scripts/encode                    # ProRes 4444 master + HEVC/AV1/H.264 into out/video
cd web && pnpm dev
```

The final render writes a half-float EXR and a 16-bit PNG per frame, about 95 GB in total.
`target/release/render post` regrades the EXRs with new look flags without retracing.
Look flags: `--exposure --bloom --streak --halation --grain --vignette --saturation`.
Look-dev flags: `--no-sky`, `--aperture`, `--spp`, `--steps`.

## How it works

- `render/src/scene.rs` is the timeline: masses, separation, orbital phase, camera path, and the
  light wash that hides the loop seam. Every physics and timeline claim there has a test.
- `render/src/shaders/trace.wgsl` integrates each ray through the superposed Schwarzschild orbit
  equation with RK4, accumulates emission from three volumetric accretion disks with Doppler
  beaming and gravitational redshift, and samples a procedural sky when it escapes.
- `render/src/shaders/post.wgsl` runs the grade: bloom pyramid, halation, anamorphic streak,
  chromatic aberration, vignette, AgX tonemap, film grain.
- `render/src/geodesic.rs` is a CPU copy of the integrator so the photon ring radius and the
  Einstein deflection angle are checked by `cargo test`.

## Development

`scripts/agent` drives the branch, worktree and pull request workflow. See
`CONTRIBUTING.md`.

## License

MIT
