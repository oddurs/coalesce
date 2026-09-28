# black-hole

A binary black hole merger scored to a song: a 220 second seamless loop in the 1.90:1 frame,
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
- The loop is 220 s, 5280 frames at 24 fps: the song (213.6 s, "Vast Sustained Peak", not in
  the repo) from frame 0, then two bars of silence. Frame 0 and frame 5280 are the same image
  by construction. The camera runs on loop time and is periodic; the seam is a dissolve in the
  silence while it looks up and away, with the opening binary at the remnant's mass until it
  is back in frame, so both sides lens the sky alike.
- The timeline is scored: 75 bpm, bars from 1.666 s. The pair eclipses (lines up with the
  camera) on the song's hits, `ECLIPSES` in `scene.rs`, and merges on the downbeat at
  174.466 s where its riser peaks; between hits the orbit speeds up smoothly. Physical time
  runs unwarped up to the merger so the eclipses stay on the beat. Keep camera keys on the
  song's landmarks; the eclipses are tested against the camera, so moving a key moves them.
- Judge timing with sound: `scripts/preview <song.wav>` renders the loop small and puts the
  song under it in `out/preview.mp4`.
- Look-dev at 1920x1010 with low samples (`scripts/render --preview --t <s>`). Final at 4096x2160.
- A look change alone is `render post` over the EXRs; only physics or geometry changes need a retrace.
- Check stills at 1:1 at 4K before trusting a look; previews hid disk aliasing and bokeh noise.

## Deploying

- The site is a static build (`adapter-static`, one prerendered page) on Vercel. Its media, the
  film with its score, lives on Cloudflare R2 at media.oddurs.com: 220 s with sound is far past
  Vercel Hobby's 100 MB upload, and R2 charges nothing for the bandwidth.
- From a finished master: `scripts/encode` writes the web set to `out/web` (`loop-av1.mp4`,
  `loop-hevc.mp4`, `loop-720.mp4`, `poster.jpg`, `hero.jpg`), then `scripts/publish` uploads it
  under content-hashed names and rewrites `web/src/lib/media.json`. Commit the manifest through a
  PR, then deploy from `web/` with `vercel deploy --prod`. Old media stays on R2, so earlier
  deploys and rollbacks keep playing.
- R2 is set up: bucket `coalesce-media` in the personal Cloudflare account (the login also sees
  MayStar Consulting, so the script pins the account), media.oddurs.com attached as its custom
  domain (zone `4b419085f866dc381c4fbace9f7281eb`), TLS 1.2 minimum. It needs `npx wrangler
  login` on a new machine. Media is cached for a year: never overwrite a published key.
- The `<source>` list is in the page's HTML, so the video starts loading before any script runs.
  Phones, reduced motion and codec choice are media queries and types there, not script. The
  sound is the film's own track, muted until the viewer asks for it.
