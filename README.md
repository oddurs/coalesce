# Coalesce

[![CI](https://github.com/oddurs/coalesce/actions/workflows/ci.yml/badge.svg)](https://github.com/oddurs/coalesce/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-black.svg)](LICENSE)

Two black holes spiral together and become one: a binary black hole inspiral, merger and
ringdown, rendered as a seamless 75 second loop in the 1.90:1 frame, and the small site that
plays it.

**Watch it:** https://black-hole-ten-liard.vercel.app

## How it is made

The renderer is a GPU ray tracer written for this piece in Rust and WGSL on `wgpu`. Ordinary
renderers trace straight rays; a black hole bends them, so every sample integrates a curved
light path through the combined field of both holes. A second pass grades the linear frames
the way film and glass would.

- `render/src/scene.rs` is the film as pure functions of loop time: the inspiral, the merger
  and ringdown, the camera path, the exposure arc, and the playback rate that slows the merger
  to a third of real time. Its claims are tests.
- `render/src/shaders/trace.wgsl` integrates each ray with RK4 through the superposed
  Schwarzschild orbit equation, accumulates emission from three volumetric accretion disks
  with Doppler beaming and gravitational redshift, lenses the sky through gravitational-wave
  ripples, and samples a procedural star field when a ray escapes. Each of a pixel's samples
  is traced at its own instant across a 180 degree shutter, which is the motion blur.
- `render/src/shaders/post.wgsl` is the grade: a bloom pyramid and halation sized to the frame,
  an anamorphic streak fed only by highlights, spectral chromatic aberration, lens falloff, a
  per-channel filmic curve with an ember split tone, and grain that lives in the midtones.
- `render/src/geodesic.rs` is a CPU copy of the integrator so the photon ring radius and the
  Einstein deflection angle are checked by `cargo test`.

The loop closes without a cut you can see. After the ringdown the camera draws back and tilts
up to the sky; while the system is out of frame a short dissolve swaps the remnant for the
binary, which carries the remnant's mass until it is back in view so both sides bend starlight
alike. The camera path is periodic, so the wrap is as smooth as any other frame.

## Requirements

Rust (pinned in `rust-toolchain.toml`), pnpm, ffmpeg, and a GPU with Metal or Vulkan.

## Quickstart

```sh
scripts/setup                     # hooks, toolchains, dependencies
scripts/task check                # format, lint, tests, build
scripts/render --preview --t 34   # one look-dev frame to out/preview/still.png
scripts/render --lo               # the whole loop at 960x506
```

The published film is 2048x1080 at 16 samples, about five hours on an M4 Pro:

```sh
cargo build --release -p render
target/release/render frames --width 2048 --height 1080 --spp 16 --steps 900 --out out/frames-1080
scripts/encode out/frames-1080 out/video-1080
```

`scripts/render` with no flags renders all 1800 frames at 4096x2160. Frames are written as a
half-float EXR and a 16-bit PNG each; `target/release/render post` regrades the EXRs with new
look flags (`--exposure --bloom --streak --halation --grain --vignette --saturation`) without
tracing again. Render output lives in `out/` and is never committed.

## The site

`web/` is a SvelteKit site built as static files: one page, a minimal player, and the video.
`scripts/encode` writes the web set (AV1 and 10-bit HEVC at 2K, H.264 at 720p for phones),
each tuned to a VMAF of about 95 against the master. Copy it into `web/static/video` and
deploy from `web/` with the Vercel CLI; see `CLAUDE.md` for the details.

```sh
cd web && pnpm dev
```

## Development

Work happens one branch per worktree through `scripts/agent`, and `main` only moves through a
merged pull request. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT, by [Oddur Sigurdsson](https://github.com/oddurs).
