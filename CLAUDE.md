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
- The loop is 75 s, 1800 frames at 24 fps. Frame 0 and frame 1800 are the same image by
  construction. The camera runs on loop time and is periodic; the seam is a short dissolve
  while it looks up and away, with the opening binary at the remnant's mass until it is
  back in frame, so both sides lens the sky alike.
- Look-dev at 1920x1010 with low samples (`scripts/render --preview --t <s>`). Final at 4096x2160.
- A look change alone is `render post` over the EXRs; only physics or geometry changes need a retrace.
- Check stills at 1:1 at 4K before trusting a look; previews hid disk aliasing and bokeh noise.

## Deploying

- The site is a static build (`adapter-static`, one prerendered page) and ships with its video.
- `scripts/encode <frames-dir> <out-dir>` writes the master and the web set: `loop-av1.mp4`,
  `loop-hevc.mp4`, `loop-720.mp4`, `poster.jpg`, `hero.jpg`. Copy the web set into `web/static/video`.
- `scripts/music <track.wav>` writes the looped music to `web/static/audio/coalesce.m4a`, also
  gitignored and also part of the upload budget (about 3.4 MB).
- Deploy from `web/` with the Vercel CLI: `vercel deploy` (preview) or `vercel deploy --prod`. The CLI
  uploads the gitignored video; a Git-connected build would not have it. Keep the upload under Vercel
  Hobby's 100 MB: the web set and music come to about 91 MB.
- Media URLs carry a hash of the file (`web/vite.config.ts`) and are cached for a year, so a new
  encode must be copied in before the build, never swapped into a finished one.
- The `<source>` list is in the page's HTML, so the video starts loading before any script runs.
  Phones, reduced motion and codec choice are media queries and types there, not script.

