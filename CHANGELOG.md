# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- GPU geodesic ray tracer for a binary black hole with volumetric accretion disks, Doppler
  beaming, gravitational redshift, gravitational-wave lensing and a procedural sky.
- A 75 second loop: inspiral, merger at a third of real speed, ringdown, and a transition that
  tilts to the sky and back with the remnant and binary matched in mass across the dissolve.
- Motion blur from a 180 degree shutter; fine gas filaments filtered to the pixel.
- Film grade: filmic curve, ember split tone, frame-relative bloom and halation, highlight-only
  anamorphic streak, spectral chromatic aberration, lens falloff, midtone grain.
- `scripts/render` and `scripts/encode`, with a web set sized for a static deploy.
- A static site with a minimal player, a landscape-first phone layout, a fade-in from black,
  and a still for reduced motion.
- The remnant's disk settles into a warped, precessing sheet with spiral arms, a ragged rim
  and a flared outer edge, and the merger flash holds filament detail instead of clipping.
- The page lists its video sources in the HTML, so the film starts loading before any script runs,
  media URLs are versioned by content and cached for a year, reduced motion downloads no video,
  and the loader returns if playback stalls.
- Music starts on the click: the track's opening is fetched ahead and decoded in milliseconds,
  and hands over sample-aligned to the whole track once it has loaded. Volume starts at 50%.
