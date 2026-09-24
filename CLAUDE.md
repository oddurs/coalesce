# black-hole

A binary black hole merger rendered as a 60 second seamless loop at 4096x2160 (1.90:1),
served as a fullscreen video by a SvelteKit site.

## Layout

- `render/` Rust + wgpu. Compute-shader geodesic ray tracer and post pass. Writes EXR
  beauty frames and graded 16-bit PNGs.
- `scripts/` `task` (the toolchain seam), `agent` (workflow), `setup`, `render`, `encode`.
- `web/` SvelteKit site that plays the encoded loop.
- `out/` render output. Ignored by git.

## Working here

- Toolchain goes through `scripts/task {fmt|fmt:check|lint|test|build|check}`. Never
  call cargo or pnpm directly from CI or hooks.
- One unit of work is one branch in its own worktree: `scripts/agent start <type>/<slug>`.
  `main` only advances through a merged PR.
- Conventional Commits, subject at most 72 characters, imperative, no trailing period.
- Never put attribution to an AI tool, model, or assistant in any commit, comment, doc,
  or PR. The hooks reject the common forms; do not route around them.
- A bug fix arrives with the test that would have caught it. Physics claims (photon
  ring radius, shadow size, timeline invariants) are tests in `render/`, not comments.
- Prefer the standard library. A dependency has to earn its place.

## Rendering

- Units: gravitational radius of the total mass is 1. Scene time is loop seconds.
- The loop is 1440 frames at 24 fps. Frame 0 and frame 1440 are the same image by
  construction; the loop point sits inside the light wash at the end of the ringdown.
- Look-dev at 1920x1010 with low samples. Final at 4096x2160.
