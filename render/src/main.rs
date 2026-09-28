use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};

use render::gpu::{Gpu, Look, Post, Quality, Tracer};
use render::image::{Hdr, write_png16};
use render::scene;

#[derive(Parser)]
#[command(name = "render", about = "Binary black hole loop renderer")]
struct Cli {
    /// Share the machine while rendering: short GPU slices with rests
    /// between them. About a third slower.
    #[arg(long, global = true)]
    gentle: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args, Clone)]
struct QualityArgs {
    #[arg(long, default_value_t = 4096)]
    width: u32,
    #[arg(long, default_value_t = 2160)]
    height: u32,
    #[arg(long, default_value_t = 16)]
    spp: u32,
    #[arg(long, default_value_t = 900)]
    steps: u32,
    /// Look-dev size: 1920x1010, fewer steps.
    #[arg(long)]
    preview: bool,
    /// Hide stars and nebula.
    #[arg(long)]
    no_sky: bool,
    /// Scale the lens aperture; 0 is a pinhole.
    #[arg(long, default_value_t = 1.0)]
    aperture: f32,
}

impl QualityArgs {
    fn quality(&self) -> Quality {
        let mut q = if self.preview {
            render::gpu::preview_quality(self.spp)
        } else {
            Quality {
                width: self.width,
                height: self.height,
                spp: self.spp,
                max_steps: self.steps,
                sky: 1.0,
                aperture: 1.0,
                wave_lens: 0.007,
                wave_ripple: 0.6,
            }
        };
        if self.no_sky {
            q.sky = 0.0;
        }
        q.aperture = self.aperture;
        q
    }
}

#[derive(clap::Args, Clone)]
struct LookArgs {
    #[arg(long)]
    exposure: Option<f32>,
    #[arg(long)]
    bloom: Option<f32>,
    #[arg(long)]
    streak: Option<f32>,
    #[arg(long)]
    halation: Option<f32>,
    #[arg(long)]
    grain: Option<f32>,
    #[arg(long)]
    vignette: Option<f32>,
    #[arg(long)]
    saturation: Option<f32>,
}

impl LookArgs {
    fn look(&self) -> Look {
        let mut l = Look::default();
        if let Some(v) = self.exposure {
            l.exposure = v;
        }
        if let Some(v) = self.bloom {
            l.bloom = v;
        }
        if let Some(v) = self.streak {
            l.streak = v;
        }
        if let Some(v) = self.halation {
            l.halation = v;
        }
        if let Some(v) = self.grain {
            l.grain = v;
        }
        if let Some(v) = self.vignette {
            l.vignette = v;
        }
        if let Some(v) = self.saturation {
            l.saturation = v;
        }
        l
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Render one frame at a loop time, for look development.
    Still {
        /// Loop time in seconds.
        #[arg(long, default_value_t = 12.0)]
        t: f32,
        #[arg(long, default_value = "out/still.png")]
        out: PathBuf,
        /// Also keep the linear EXR next to the PNG.
        #[arg(long)]
        exr: bool,
        #[command(flatten)]
        quality: QualityArgs,
        #[command(flatten)]
        look: LookArgs,
    },
    /// Render a frame range of the loop. Skips frames that already exist.
    Frames {
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = scene::LOOP_FRAMES)]
        end: u32,
        #[arg(long, default_value = "out/frames")]
        out: PathBuf,
        /// Graded PNGs only. Previews are never re-graded, and the EXRs are
        /// most of their size.
        #[arg(long)]
        no_exr: bool,
        #[command(flatten)]
        quality: QualityArgs,
        #[command(flatten)]
        look: LookArgs,
    },
    /// Re-grade existing EXR frames without re-tracing.
    Post {
        #[arg(long, default_value = "out/frames")]
        input: PathBuf,
        #[arg(long, default_value = "out/frames")]
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = scene::LOOP_FRAMES)]
        end: u32,
        #[command(flatten)]
        look: LookArgs,
    },
}

struct Session {
    gpu: Gpu,
    tracer: Tracer,
    post: Post,
}

impl Session {
    fn new(gentle: bool) -> Result<Self, String> {
        let gpu = Gpu::new()?;
        eprintln!("gpu: {}", gpu.adapter_name);
        let mut tracer = Tracer::new(&gpu);
        if gentle {
            tracer.gentle();
        }
        let post = Post::new(&gpu);
        Ok(Session { gpu, tracer, post })
    }

    /// Trace loop time `t`, one sample per instant across the shutter. Inside
    /// the seam window both sides of the cut are traced and mixed in linear
    /// light.
    fn trace(&mut self, t: f32, q: &Quality) -> Result<Hdr, String> {
        let t = t.rem_euclid(scene::LOOP_SECONDS);
        let times = scene::shutter_times(t, q.spp);
        let k = scene::dissolve(t);
        if k <= 0.0 || k >= 1.0 {
            let frames: Vec<_> = times.iter().map(|&s| scene::frame(s)).collect();
            let accum = self.tracer.render(&self.gpu, &frames, q)?;
            return Ok(Hdr::from_rgba(q.width, q.height, &accum));
        }
        let before: Vec<_> = times.iter().map(|&s| scene::frame_at(s, s)).collect();
        let after: Vec<_> = times
            .iter()
            .map(|&s| scene::frame_at(s, s - scene::LOOP_SECONDS))
            .collect();
        let a = Hdr::from_rgba(
            q.width,
            q.height,
            &self.tracer.render(&self.gpu, &before, q)?,
        );
        let b = Hdr::from_rgba(
            q.width,
            q.height,
            &self.tracer.render(&self.gpu, &after, q)?,
        );
        let rgb = a
            .rgb
            .iter()
            .zip(&b.rgb)
            .map(|(x, y)| x * (1.0 - k) + y * k)
            .collect();
        Ok(Hdr {
            width: q.width,
            height: q.height,
            rgb,
        })
    }

    fn grade(&self, hdr: &Hdr, t: f32, index: u32, look: &Look) -> Result<Vec<f32>, String> {
        self.post.process(
            &self.gpu,
            hdr,
            look,
            scene::flash(scene::physical_time(t)),
            scene::exposure(scene::physical_time(t)),
            index,
        )
    }
}

/// Luminance percentiles of a linear frame, for look development.
fn stats(hdr: &Hdr) -> String {
    let mut lum: Vec<f32> = hdr
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
        .collect();
    lum.sort_by(|a, b| a.total_cmp(b));
    let pct = |q: f32| lum[((lum.len() - 1) as f32 * q) as usize];
    format!(
        "lum p1 {:.4} p25 {:.4} p50 {:.4} p75 {:.4} p90 {:.4} p99 {:.3} max {:.2}",
        pct(0.01),
        pct(0.25),
        pct(0.5),
        pct(0.75),
        pct(0.9),
        pct(0.99),
        lum[lum.len() - 1]
    )
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Still {
            t,
            out,
            exr,
            quality,
            look,
        } => {
            let q = quality.quality();
            let mut s = Session::new(cli.gentle)?;
            let started = Instant::now();
            let hdr = s.trace(t, &q)?;
            eprintln!(
                "traced {}x{} @ {} spp in {:.1?}",
                q.width,
                q.height,
                q.spp,
                started.elapsed()
            );
            eprintln!("{}", stats(&hdr));
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            if exr {
                hdr.write_exr(&out.with_extension("exr"))?;
            }
            let graded = s.grade(&hdr, t, (t * scene::FPS) as u32, &look.look())?;
            write_png16(&out, q.width, q.height, &graded)?;
            eprintln!("wrote {}", out.display());
        }
        Cmd::Frames {
            start,
            end,
            out,
            no_exr,
            quality,
            look,
        } => {
            let q = quality.quality();
            let look = look.look();
            let mut s = Session::new(cli.gentle)?;
            let exr_dir = out.join("exr");
            let png_dir = out.join("png");
            if !no_exr {
                std::fs::create_dir_all(&exr_dir).map_err(|e| e.to_string())?;
            }
            std::fs::create_dir_all(&png_dir).map_err(|e| e.to_string())?;
            let total = end.saturating_sub(start);
            let started = Instant::now();
            let mut done = 0u32;
            for n in start..end {
                let png = png_dir.join(format!("frame_{n:04}.png"));
                let exr = exr_dir.join(format!("frame_{n:04}.exr"));
                if png.exists() && (no_exr || exr.exists()) {
                    continue;
                }
                let t = scene::frame_time(n);
                let frame_started = Instant::now();
                let hdr = s.trace(t, &q)?;
                if !no_exr {
                    hdr.write_exr(&exr)?;
                }
                let graded = s.grade(&hdr, t, n, &look)?;
                write_png16(&png, q.width, q.height, &graded)?;
                done += 1;
                let per = started.elapsed().as_secs_f32() / done as f32;
                let remaining = (total - (n - start + 1)) as f32 * per;
                eprintln!(
                    "frame {n:04} t={t:6.2}s {:.1}s  eta {:.0} min",
                    frame_started.elapsed().as_secs_f32(),
                    remaining / 60.0
                );
            }
        }
        Cmd::Post {
            input,
            out,
            start,
            end,
            look,
        } => {
            let look = look.look();
            let s = Session::new(cli.gentle)?;
            let png_dir = out.join("png");
            std::fs::create_dir_all(&png_dir).map_err(|e| e.to_string())?;
            for n in start..end {
                let exr = input.join("exr").join(format!("frame_{n:04}.exr"));
                let hdr = Hdr::read_exr(&exr)?;
                let t = scene::frame_time(n);
                let graded = s.grade(&hdr, t, n, &look)?;
                write_png16(
                    &png_dir.join(format!("frame_{n:04}.png")),
                    hdr.width,
                    hdr.height,
                    &graded,
                )?;
                eprintln!("graded {n:04}");
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
