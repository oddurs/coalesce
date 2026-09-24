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
scripts/render --preview   # one 1080p frame to out/preview
scripts/render             # all 1440 frames at 4K to out/frames
scripts/encode             # ProRes master + web encodes into out/video
cd web && pnpm dev
```

## Development

`scripts/agent` drives the branch, worktree and pull request workflow. See
`CONTRIBUTING.md`.

## License

MIT
