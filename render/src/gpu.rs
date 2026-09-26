//! wgpu driver: the trace pipeline that accumulates samples into a linear HDR
//! buffer, and the post pipeline that turns one HDR frame into display pixels.

use std::borrow::Cow;
use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::blackbody;
use crate::image::Hdr;
use crate::scene::{self, Frame, cross, normalize};

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("black-hole"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("device: {e}"))?;
        Ok(Gpu {
            device,
            queue,
            adapter_name,
        })
    }

    fn wait(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|e| format!("gpu poll: {e}"))
    }

    /// Copy a GPU buffer back to the CPU as f32s.
    fn read_back(&self, src: &wgpu::Buffer, bytes: u64) -> Result<Vec<f32>, String> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, bytes);
        self.queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.wait()?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("map: {e}"))?;
        let view = slice
            .get_mapped_range()
            .map_err(|e| format!("mapped range: {e}"))?;
        let out: Vec<f32> = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        staging.unmap();
        Ok(out)
    }
}

fn storage_buffer(device: &wgpu::Device, label: &str, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.max(16),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn layout_entry(
    binding: u32,
    ty: wgpu::BufferBindingType,
    dynamic: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

// ------------------------------------------------------------------ trace

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuBody {
    pos_rs: [f32; 4],
    vel_mass: [f32; 4],
    disk: [f32; 4],
    normal: [f32; 4],
    tangent: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TraceParams {
    cam_pos: [f32; 4],
    cam_u: [f32; 4],
    cam_v: [f32; 4],
    cam_w: [f32; 4],
    res: [u32; 4],
    misc: [f32; 4],
    big: [f32; 4],
    sky: [f32; 4],
    gw: [f32; 4],
    gw2: [f32; 4],
    spiral: [f32; 4],
    bodies: [GpuBody; 2],
}

const TRACE_STRIDE: u64 = 512;
const TILE: u32 = 512;

#[derive(Clone, Copy, Debug)]
pub struct Quality {
    pub width: u32,
    pub height: u32,
    pub spp: u32,
    pub max_steps: u32,
    /// 0 hides stars and nebula, for isolating the disks in look-dev.
    pub sky: f32,
    /// Multiplier on the camera aperture.
    pub aperture: f32,
    /// Gravitational-wave lensing and disk ripple strengths.
    pub wave_lens: f32,
    pub wave_ripple: f32,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn v4(v: [f32; 3], w: f32) -> [f32; 4] {
    [v[0], v[1], v[2], w]
}

fn trace_params(frame: &Frame, q: &Quality) -> TraceParams {
    let cam = &frame.camera;
    let fwd = normalize(sub(cam.look_at, cam.pos));
    let right0 = normalize(cross(fwd, cam.up));
    let up0 = cross(right0, fwd);
    // Roll about the view axis.
    let (sr, cr) = cam.roll_deg.to_radians().sin_cos();
    let right = normalize([
        right0[0] * cr + up0[0] * sr,
        right0[1] * cr + up0[1] * sr,
        right0[2] * cr + up0[2] * sr,
    ]);
    let up = cross(right, fwd);
    let tan_x = (cam.fov_x_deg.to_radians() * 0.5).tan();
    let tan_y = tan_x * q.height as f32 / q.width as f32;
    let px_rad = 2.0 * tan_x / q.width as f32;
    let bodies = frame.bodies.map(|b| {
        let n = normalize(b.disk_normal);
        let seed = [n[2], n[0], -n[1]];
        let tangent = normalize(cross(n, seed));
        GpuBody {
            pos_rs: v4(b.pos, b.rs),
            vel_mass: v4(b.vel, b.mass),
            disk: [b.disk_inner, b.disk_outer, b.disk_gain, b.disk_temp],
            normal: v4(n, 0.0),
            tangent: v4(tangent, 0.0),
        }
    });
    TraceParams {
        cam_pos: v4(cam.pos, cam.focus_distance),
        cam_u: v4(right, tan_x),
        cam_v: v4(up, tan_y),
        cam_w: v4(fwd, cam.aperture * q.aperture),
        res: [q.width, q.height, 0, 0],
        misc: [frame.tau, 0.0, q.max_steps as f32, 0.0],
        big: [
            frame.circumbinary_inner,
            frame.circumbinary_outer,
            frame.circumbinary_gain,
            3900.0,
        ],
        sky: [q.sky, q.sky * 0.3, px_rad * 1.1, px_rad],
        gw: [
            scene::GW_SPEED,
            scene::GW_T0,
            scene::GW_DT,
            scene::GW_LEN as f32,
        ],
        gw2: [
            q.wave_lens,
            q.wave_ripple,
            frame.flash,
            if frame.merged { 0.0 } else { 1.0 },
        ],
        spiral: [frame.spiral_phase, 0.45, frame.lump_phase, 0.6],
        bodies,
    }
}

pub struct Tracer {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    lut: wgpu::Buffer,
    gw: wgpu::Buffer,
    params: wgpu::Buffer,
    params_capacity: u64,
}

impl Tracer {
    pub fn new(gpu: &Gpu) -> Self {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("trace"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/trace.wgsl"))),
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("trace"),
                entries: &[
                    layout_entry(0, wgpu::BufferBindingType::Uniform, true),
                    layout_entry(
                        1,
                        wgpu::BufferBindingType::Storage { read_only: false },
                        false,
                    ),
                    layout_entry(
                        2,
                        wgpu::BufferBindingType::Storage { read_only: true },
                        false,
                    ),
                    layout_entry(
                        3,
                        wgpu::BufferBindingType::Storage { read_only: true },
                        false,
                    ),
                ],
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("trace"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("trace"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let lut = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("blackbody"),
                contents: bytemuck::cast_slice(&blackbody::lut()),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let gw = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gw table"),
                contents: bytemuck::cast_slice(&scene::gw_table()),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params_capacity = 64;
        let params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trace params"),
            size: params_capacity * TRACE_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Tracer {
            pipeline,
            layout,
            lut,
            gw,
            params,
            params_capacity,
        }
    }

    /// Render one image. Sample `s` is traced through `frames[s % len]`, so
    /// instants across the shutter give motion blur for free. Returns
    /// premultiplied RGBA accumulation: rgb sums and the sample count in alpha.
    pub fn render(&mut self, gpu: &Gpu, frames: &[Frame], q: &Quality) -> Result<Vec<f32>, String> {
        if frames.is_empty() {
            return Err("render needs at least one frame".into());
        }
        let pixels = (q.width * q.height) as u64;
        let accum = storage_buffer(&gpu.device, "accum", pixels * 16);
        let tiles_x = q.width.div_ceil(TILE);
        let tiles_y = q.height.div_ceil(TILE);
        let tiles = (tiles_x * tiles_y) as u64;
        if tiles > self.params_capacity {
            self.params_capacity = tiles;
            self.params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("trace params"),
                size: tiles * TRACE_STRIDE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trace"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params,
                        offset: 0,
                        size: NonZeroU64::new(std::mem::size_of::<TraceParams>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: accum.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.lut.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.gw.as_entire_binding(),
                },
            ],
        });

        for sample in 0..q.spp {
            let base = trace_params(&frames[sample as usize % frames.len()], q);
            // One submit per sample keeps each command buffer short.
            let mut staged = vec![0u8; (tiles * TRACE_STRIDE) as usize];
            for ty in 0..tiles_y {
                for tx in 0..tiles_x {
                    let mut p = base;
                    p.res[2] = tx * TILE;
                    p.res[3] = ty * TILE;
                    p.misc[1] = sample as f32;
                    let i = ((ty * tiles_x + tx) as u64 * TRACE_STRIDE) as usize;
                    staged[i..i + std::mem::size_of::<TraceParams>()]
                        .copy_from_slice(bytemuck::bytes_of(&p));
                }
            }
            gpu.queue.write_buffer(&self.params, 0, &staged);
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                for t in 0..tiles {
                    pass.set_bind_group(0, &bind, &[(t * TRACE_STRIDE) as u32]);
                    pass.dispatch_workgroups(TILE / 8, TILE / 8, 1);
                }
            }
            gpu.queue.submit(Some(enc.finish()));
            gpu.wait()?;
        }
        gpu.read_back(&accum, pixels * 16)
    }
}

// ------------------------------------------------------------------ post

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PassParams {
    src_size: [u32; 2],
    dst_size: [u32; 2],
    src_off: u32,
    dst_off: u32,
    src_buf: u32,
    dst_buf: u32,
    mode: u32,
    frame: u32,
    pad: [u32; 2],
    f: [f32; 4],
    g: [f32; 4],
    h: [f32; 4],
    k: [f32; 4],
    levels: [[u32; 4]; 6],
}

const PASS_STRIDE: u64 = 256;
const LEVELS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub exposure: f32,
    pub bloom: f32,
    pub streak: f32,
    pub halation: f32,
    pub aberration: f32,
    pub vignette: f32,
    pub grain: f32,
    pub distortion: f32,
    pub saturation: f32,
    pub knee: f32,
    /// Scene-linear luminance a highlight must exceed to feed the streak.
    pub streak_threshold: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            exposure: 1.1,
            bloom: 0.5,
            streak: 0.5,
            halation: 0.22,
            aberration: 0.0015,
            vignette: 0.35,
            grain: 0.028,
            distortion: 0.04,
            saturation: 1.22,
            knee: 0.6,
            streak_threshold: 2.0,
        }
    }
}

pub struct Post {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
}

#[derive(Clone, Copy)]
struct Level {
    w: u32,
    h: u32,
    off: u32,
}

impl Post {
    pub fn new(gpu: &Gpu) -> Self {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("post"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/post.wgsl"))),
            });
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("post"),
                entries: &[
                    layout_entry(0, wgpu::BufferBindingType::Uniform, true),
                    layout_entry(
                        1,
                        wgpu::BufferBindingType::Storage { read_only: true },
                        false,
                    ),
                    layout_entry(2, rw, false),
                    layout_entry(3, rw, false),
                    layout_entry(4, rw, false),
                    layout_entry(5, rw, false),
                ],
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("post"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("post"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post params"),
            size: 64 * PASS_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Post {
            pipeline,
            layout,
            params,
        }
    }

    /// Grade one linear RGB frame into display-referred sRGB RGBA floats.
    pub fn process(
        &self,
        gpu: &Gpu,
        hdr: &Hdr,
        look: &Look,
        flash: f32,
        exposure: f32,
        frame_index: u32,
    ) -> Result<Vec<f32>, String> {
        let (width, height) = (hdr.width, hdr.height);
        let mut levels = Vec::with_capacity(LEVELS);
        let mut off = 0u32;
        let (mut w, mut h) = (width, height);
        for _ in 0..LEVELS {
            w = w.div_ceil(2);
            h = h.div_ceil(2);
            levels.push(Level { w, h, off });
            off += w * h;
        }
        let pixels = (width * height) as u64;
        let rgba: Vec<f32> = hdr
            .rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 1.0])
            .collect();
        let src = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("post src"),
                contents: bytemuck::cast_slice(&rgba),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let mips = storage_buffer(&gpu.device, "mips", off as u64 * 16);
        let scratch = storage_buffer(
            &gpu.device,
            "scratch",
            (levels[0].w * levels[0].h) as u64 * 16,
        );
        let streak = storage_buffer(
            &gpu.device,
            "streak",
            (levels[0].w * levels[0].h) as u64 * 16,
        );
        let dst = storage_buffer(&gpu.device, "post dst", pixels * 16);
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params,
                        offset: 0,
                        size: NonZeroU64::new(std::mem::size_of::<PassParams>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: src.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mips.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: scratch.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: streak.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: dst.as_entire_binding(),
                },
            ],
        });

        let mut passes: Vec<PassParams> = Vec::new();
        let blank = PassParams {
            src_size: [0; 2],
            dst_size: [0; 2],
            src_off: 0,
            dst_off: 0,
            src_buf: 0,
            dst_buf: 0,
            mode: 0,
            frame: frame_index,
            pad: [0; 2],
            f: [0.0; 4],
            g: [0.0; 4],
            h: [0.0; 4],
            k: [0.0; 4],
            levels: [[0; 4]; 6],
        };
        // Pyramid: downsample, then a separable blur at each level.
        let mut prev = (0u32, 0u32, width, height);
        for (i, lv) in levels.iter().enumerate() {
            let knee = if i == 0 { look.knee } else { 0.0 };
            passes.push(PassParams {
                src_size: [prev.2, prev.3],
                dst_size: [lv.w, lv.h],
                src_off: prev.1,
                dst_off: lv.off,
                src_buf: prev.0,
                dst_buf: 1,
                mode: 0,
                f: [knee, 0.0, 0.0, 0.0],
                ..blank
            });
            // Radii scale with the frame so a preview glows like the 4K final;
            // fixed pixel radii made the 4K glow half as wide.
            let radius = ((4 + i) as f32 * width as f32 / 1920.0).round().max(1.0) as u32;
            passes.push(PassParams {
                src_size: [lv.w, lv.h],
                dst_size: [lv.w, lv.h],
                src_off: lv.off,
                dst_off: 0,
                src_buf: 1,
                dst_buf: 2,
                mode: 1,
                f: [1.0, 0.0, radius as f32, radius as f32 * 0.5],
                ..blank
            });
            passes.push(PassParams {
                src_size: [lv.w, lv.h],
                dst_size: [lv.w, lv.h],
                src_off: 0,
                dst_off: lv.off,
                src_buf: 2,
                dst_buf: 1,
                mode: 1,
                f: [0.0, 1.0, radius as f32, radius as f32 * 0.5],
                ..blank
            });
            prev = (1, lv.off, lv.w, lv.h);
        }
        // Anamorphic streak: only highlights far above the gas, at half
        // resolution, blurred sideways over a fixed fraction of the frame so
        // previews and 4K flare alike. Sourced from all light, it veiled the
        // frame; sourced from a coarse level, it averaged the stars away.
        let s = levels[0];
        passes.push(PassParams {
            src_size: [width, height],
            dst_size: [s.w, s.h],
            src_buf: 0,
            dst_buf: 3,
            mode: 0,
            f: [0.0, look.streak_threshold, 0.0, 0.0],
            ..blank
        });
        let radius = (s.w as f32 * 0.03).round().max(4.0);
        for (from, to) in [(3u32, 2u32), (2, 3)] {
            passes.push(PassParams {
                src_size: [s.w, s.h],
                dst_size: [s.w, s.h],
                src_buf: from,
                dst_buf: to,
                mode: 1,
                f: [1.0, 0.0, radius, radius * 0.55],
                ..blank
            });
        }
        let mut level_table = [[0u32; 4]; 6];
        for (i, lv) in levels.iter().enumerate() {
            level_table[i] = [lv.w, lv.h, lv.off, 0];
        }
        passes.push(PassParams {
            src_size: [width, height],
            dst_size: [width, height],
            src_buf: 0,
            dst_buf: 4,
            mode: 2,
            g: [look.exposure * exposure, 0.0, look.bloom, look.streak],
            h: [look.halation, look.aberration, look.vignette, look.grain],
            k: [look.distortion, look.saturation, flash, 0.0],
            levels: level_table,
            ..blank
        });

        let mut staged = vec![0u8; passes.len() * PASS_STRIDE as usize];
        for (i, p) in passes.iter().enumerate() {
            let o = i * PASS_STRIDE as usize;
            staged[o..o + std::mem::size_of::<PassParams>()].copy_from_slice(bytemuck::bytes_of(p));
        }
        gpu.queue.write_buffer(&self.params, 0, &staged);
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut cp = enc.begin_compute_pass(&Default::default());
            cp.set_pipeline(&self.pipeline);
            for (i, p) in passes.iter().enumerate() {
                cp.set_bind_group(0, &bind, &[(i as u64 * PASS_STRIDE) as u32]);
                cp.dispatch_workgroups(p.dst_size[0].div_ceil(8), p.dst_size[1].div_ceil(8), 1);
            }
        }
        gpu.queue.submit(Some(enc.finish()));
        gpu.wait()?;
        gpu.read_back(&dst, pixels * 16)
    }
}

/// Preview-sized quality for look development.
pub fn preview_quality(spp: u32) -> Quality {
    Quality {
        width: 1920,
        height: 1010,
        spp,
        max_steps: 700,
        sky: 1.0,
        aperture: 1.0,
        wave_lens: 0.007,
        wave_ripple: 0.6,
    }
}
