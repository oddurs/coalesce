"""Render the final loop on Modal GPUs instead of this machine.

Each container builds the renderer, renders one chunk of 4K frames on an
NVIDIA L40S (Vulkan through wgpu, the same code as here), folds them into a
ProRes 422 chunk and saves it to the `coalesce-chunks` volume. Chunks already
there are skipped, so a run resumes. An L40S traces a 4K frame in about 5 s;
the image matches this machine's to the grain (PSNR 41 dB, SSIM 0.995).

    uvx modal run scripts/farm.py                 # every chunk
    uvx modal run scripts/farm.py --only 0        # one chunk, as a check
    uvx modal volume get coalesce-chunks / out/chunks
    scripts/render                                 # joins them into out/master.mov

Chunks are 120 frames, so each one's name is also a name scripts/render gives
a chunk, and it treats them as done and only joins and checks them.
"""

import pathlib
import subprocess
import time

import modal

ROOT = pathlib.Path(__file__).resolve().parent.parent
# LOOP_FRAMES in render/src/scene.rs.
TOTAL = 5280
CHUNK = 120

app = modal.App("coalesce-farm")
volume = modal.Volume.from_name("coalesce-chunks", create_if_missing=True)
image = (
    modal.Image.debian_slim()
    # The Vulkan loader, and the X11 libraries NVIDIA's Vulkan driver links
    # against even with no display: without them it silently fails to load.
    .apt_install(
        "libvulkan1", "libx11-6", "libxext6", "libglvnd0", "libegl1",
        "ffmpeg", "curl", "build-essential", "pkg-config",
    )
    .env(
        {
            # The graphics side of the driver, not just compute.
            "NVIDIA_DRIVER_CAPABILITIES": "all",
            "PATH": "/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin",
        }
    )
    .run_commands(
        "curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.98.1"
    )
    .add_local_file(ROOT / "Cargo.toml", "/src/Cargo.toml", copy=True)
    .add_local_file(ROOT / "Cargo.lock", "/src/Cargo.lock", copy=True)
    .add_local_file(ROOT / "rust-toolchain.toml", "/src/rust-toolchain.toml", copy=True)
    .add_local_dir(ROOT / "render", "/src/render", copy=True, ignore=["target"])
    .run_commands("cd /src && cargo build --release -q -p render")
)


@app.function(gpu="L40S", image=image, volumes={"/chunks": volume}, timeout=3 * 3600)
def render_chunk(start: int) -> str:
    name = f"chunk_{start:04d}.mov"
    done = pathlib.Path("/chunks") / name
    if done.exists():
        return f"{name}: already rendered"
    started = time.time()
    end = min(start + CHUNK, TOTAL)
    frames = pathlib.Path("/tmp/frames")
    subprocess.run(
        ["/src/target/release/render", "frames", "--start", str(start), "--end", str(end),
         "--no-exr", "--out", str(frames)],
        check=True, capture_output=True,
    )
    part = done.with_suffix(".part")
    # The same ProRes 422 and tags as scripts/render.
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-framerate", "24", "-start_number", str(start),
         "-i", str(frames / "png" / "frame_%04d.png"), "-frames:v", str(end - start),
         "-c:v", "prores_ks", "-profile:v", "2", "-pix_fmt", "yuv422p10le", "-vendor", "apl0",
         "-color_primaries", "bt709", "-color_trc", "iec61966-2-1", "-colorspace", "bt709",
         "-f", "mov", str(part)],
        check=True,
    )
    part.rename(done)
    volume.commit()
    return f"{name}: {end - start} frames in {time.time() - started:.0f} s"


@app.local_entrypoint()
def main(only: int = -1):
    starts = [only] if only >= 0 else list(range(0, TOTAL, CHUNK))
    for line in render_chunk.map(starts):
        print(line, flush=True)
