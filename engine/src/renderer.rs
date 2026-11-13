use std::sync::Arc;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::assets::{
    load_embedded_assets, ATLAS_COLS, ATLAS_H, ATLAS_W, FRAME_NAMES, SPRITE_H, SPRITE_W,
};
use crate::bone2d::Skeleton2D;
use crate::font::{build_font_atlas, glyph_uv, CELL_H, CELL_W};
use crate::map::{cast_ray, MAP_DATA, MAP_H, MAP_W, TILE_COVER, TILE_WALL};
use crate::unit::{Enemy, EnemyState, ScreenBounds};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UnitVertex {
    local_pos: [f32; 2],
    bone_weights: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenUniform {
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UnitUniform {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    params: [f32; 4],
    clip_info: [f32; 4],
    bone_rows: [[f32; 4]; 8],
}

pub struct PlayerRenderState {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub hp: i32,
    pub ammo: i32,
    pub reserve_ammo: i32,
    pub crouch_blend: f32,
    pub aiming: bool,
    pub recoil: f32,
    pub reloading: bool,
    pub bob_x: f32,
    pub bob_y: f32,
    pub damage_flash: f32,
}

pub struct PopupText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: [f32; 4],
    pub life: f32,
}

pub struct GpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub width: u32,
    pub height: u32,

    quad_pipeline: wgpu::RenderPipeline,
    unit_pipeline: wgpu::RenderPipeline,

    screen_buf: wgpu::Buffer,
    screen_bg: wgpu::BindGroup,

    white_bg: wgpu::BindGroup,
    wall_bg: wgpu::BindGroup,
    cover_bg: wgpu::BindGroup,
    unit_atlas_bg: wgpu::BindGroup,
    weapon_bg: wgpu::BindGroup,
    font_bg: wgpu::BindGroup,

    unit_vbuf: wgpu::Buffer,
    unit_ibuf: wgpu::Buffer,
    unit_index_count: u32,

    unit_ubuf: wgpu::Buffer,
    unit_ubuf_bg: wgpu::BindGroup,
    unit_stride: usize,
    max_units: usize,
}

fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    rgba: &[u8],
    w: u32,
    h: u32,
    label: &str,
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: Some(h),
        },
        size,
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn build_unit_mesh() -> (Vec<UnitVertex>, Vec<u16>) {
    let cols: u16 = 4;
    let rows: u16 = 16;
    let mut verts = Vec::new();
    let mut indices = Vec::new();

    for r in 0..=rows {
        let v = r as f32 / rows as f32;
        // Weight distribution for the 4 bones: root(0), torso(1), head(2), coat_hem(3)
        let w_head = ((0.28 - v) / 0.18).clamp(0.0, 1.0);
        let w_coat = if v > 0.45 && v < 0.90 {
            (1.0 - ((v - 0.675).abs() / 0.225)).clamp(0.0, 1.0) * 0.55
        } else {
            0.0
        };
        let w_root = ((v - 0.65) / 0.35).clamp(0.0, 1.0);
        let w_torso = (1.0 - w_head - w_coat - w_root).max(0.0);
        let sum = (w_root + w_torso + w_head + w_coat).max(1e-5);
        let weights = [w_root / sum, w_torso / sum, w_head / sum, w_coat / sum];

        for c in 0..=cols {
            let u = c as f32 / cols as f32;
            verts.push(UnitVertex {
                local_pos: [u, v],
                bone_weights: weights,
            });
        }
    }

    let stride = cols + 1;
    for r in 0..rows {
        for c in 0..cols {
            let i0 = r * stride + c;
            let i1 = i0 + 1;
            let i2 = i0 + stride;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }
    (verts, indices)
}

impl GpuRenderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let (width, height) = if size.width == 0 || size.height == 0 {
            (960, 540)
        } else {
            (size.width, size.height)
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window).expect("create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("request adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("tactical2d_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("request device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let unit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("unit_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<UnitUniform>() as u64),
                },
                count: None,
            }],
        });

        let screen_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("screen_buf"),
            contents: bytemuck::bytes_of(&ScreenUniform {
                viewport: [width as f32, height as f32, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen_bg"),
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buf.as_entire_binding(),
            }],
        });

        let blend = Some(wgpu::BlendState::ALPHA_BLENDING);

        let quad_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad_pipe_layout"),
            bind_group_layouts: &[&screen_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_pipeline"),
            layout: Some(&quad_pipe_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_quad"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_quad"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let unit_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("unit_pipe_layout"),
            bind_group_layouts: &[&screen_layout, &tex_layout, &unit_layout],
            push_constant_ranges: &[],
        });
        let unit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("unit_pipeline"),
            layout: Some(&unit_pipe_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_unit"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UnitVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_unit"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let assets = load_embedded_assets();
        let font_atlas = build_font_atlas();

        let white_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_nearest,
            &[255, 255, 255, 255],
            1,
            1,
            "white_tex",
        );
        let wall_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_linear,
            assets.wall_tex.as_raw(),
            assets.wall_tex.width(),
            assets.wall_tex.height(),
            "wall_tex",
        );
        let cover_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_linear,
            assets.cover_tex.as_raw(),
            assets.cover_tex.width(),
            assets.cover_tex.height(),
            "cover_tex",
        );
        let unit_atlas_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_linear,
            assets.unit_atlas.as_raw(),
            ATLAS_W,
            ATLAS_H,
            "unit_atlas",
        );
        let weapon_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_linear,
            assets.weapon_tex.as_raw(),
            assets.weapon_tex.width(),
            assets.weapon_tex.height(),
            "weapon_tex",
        );
        let font_bg = upload_texture(
            &device,
            &queue,
            &tex_layout,
            &sampler_nearest,
            font_atlas.as_raw(),
            font_atlas.width(),
            font_atlas.height(),
            "font_atlas",
        );

        let (u_verts, u_indices) = build_unit_mesh();
        let unit_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("unit_vbuf"),
            contents: bytemuck::cast_slice(&u_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let unit_ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("unit_ibuf"),
            contents: bytemuck::cast_slice(&u_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let align = device.limits().min_uniform_buffer_offset_alignment.max(256) as usize;
        let u_size = std::mem::size_of::<UnitUniform>();
        let unit_stride = ((u_size + align - 1) / align) * align;
        let max_units = 32;

        let unit_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("unit_ubuf"),
            size: (unit_stride * max_units) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let unit_ubuf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("unit_ubuf_bg"),
            layout: &unit_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &unit_ubuf,
                    offset: 0,
                    size: wgpu::BufferSize::new(u_size as u64),
                }),
            }],
        });

        Self {
            surface,
            device,
            queue,
            config,
            width,
            height,
            quad_pipeline,
            unit_pipeline,
            screen_buf,
            screen_bg,
            white_bg,
            wall_bg,
            cover_bg,
            unit_atlas_bg,
            weapon_bg,
            font_bg,
            unit_vbuf,
            unit_ibuf,
            unit_index_count: u_indices.len() as u32,
            unit_ubuf,
            unit_ubuf_bg,
            unit_stride,
            max_units,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.queue.write_buffer(
            &self.screen_buf,
            0,
            bytemuck::bytes_of(&ScreenUniform {
                viewport: [width as f32, height as f32, 0.0, 0.0],
            }),
        );
    }

    pub fn render(
        &mut self,
        player: &PlayerRenderState,
        enemies: &mut [Enemy],
        weapon_skel: &Skeleton2D,
        popups: &[PopupText],
        time: f32,
        show_debug: bool,
        banner_text: Option<&str>,
    ) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let w = self.width as f32;
        let h = self.height as f32;
        let base_fov = if player.aiming {
            std::f32::consts::PI / 4.2
        } else {
            std::f32::consts::PI / 2.8
        };
        let focal_dist = (w * 0.5) / (base_fov * 0.5).tan();
        let crouch_offset = player.crouch_blend * (h * 0.14);
        let horizon = h * 0.5 + crouch_offset + player.bob_y * 0.5;
        let eye_height_factor = 0.5 - player.crouch_blend * 0.24;

        let mut solid_verts: Vec<QuadVertex> = Vec::with_capacity(1024);
        let mut wall_verts: Vec<QuadVertex> = Vec::with_capacity(4096);
        let mut cover_verts: Vec<QuadVertex> = Vec::with_capacity(2048);
        let mut ui_solid_verts: Vec<QuadVertex> = Vec::with_capacity(2048);
        let mut font_verts: Vec<QuadVertex> = Vec::with_capacity(4096);
        let mut strip_verts: Vec<QuadVertex> = Vec::with_capacity(200);

        // 1. Sky & Ground
        push_quad_grad(
            &mut solid_verts,
            0.0,
            0.0,
            w,
            horizon,
            [0.62, 0.70, 0.76, 1.0],
            [0.95, 0.93, 0.89, 1.0],
        );
        // Distant parallax hills
        let angle_off = (player.angle / std::f32::consts::TAU) * w * 2.0;
        let step_x = 24.0_f32;
        let mut x = 0.0_f32;
        while x < w {
            let wx = x + angle_off;
            let mh = (wx * 0.007).sin() * 28.0 + (wx * 0.017).cos() * 15.0 + 38.0;
            push_quad(
                &mut solid_verts,
                x,
                horizon - mh,
                step_x + 1.0,
                mh,
                [0.0, 0.0, 1.0, 1.0],
                [0.56, 0.64, 0.60, 1.0],
            );
            x += step_x;
        }
        push_quad_grad(
            &mut solid_verts,
            0.0,
            horizon,
            w,
            h - horizon,
            [0.55, 0.59, 0.46, 1.0],
            [0.22, 0.24, 0.20, 1.0],
        );

        // 2. 2.5D Raycasted Walls & Low Cover
        let col_w = 2.0_f32;
        let num_cols = (w / col_w).ceil() as usize;
        let mut z_buffer = vec![0.0_f32; num_cols];
        let mut cover_z_buffer = vec![0.0_f32; num_cols];

        for i in 0..num_cols {
            let sx = i as f32 * col_w + col_w * 0.5;
            let ray_angle = player.angle + (sx - w * 0.5).atan2(focal_dist);
            let cos_fix = (ray_angle - player.angle).cos();

            let wall_hit = cast_ray(player.x, player.y, ray_angle, 26.0, true);
            let wall_dist = wall_hit.dist * cos_fix;
            z_buffer[i] = wall_dist;

            let wall_h = (1.0 / wall_dist.max(0.05)) * focal_dist;
            let wall_bottom = horizon + wall_h * eye_height_factor;
            let wall_top = wall_bottom - wall_h;

            let mut u = if wall_hit.side == 0 {
                wall_hit.hit_y
            } else {
                wall_hit.hit_x
            };
            u -= u.floor();
            let shade = (1.0 - (wall_dist / 22.0).min(0.65))
                * if wall_hit.side == 1 { 0.84 } else { 1.0 };

            push_quad(
                &mut wall_verts,
                i as f32 * col_w,
                wall_top,
                col_w,
                wall_h,
                [u, 0.0, u + 0.008, 1.0],
                [shade, shade, shade, 1.0],
            );

            let cover_hit = cast_ray(player.x, player.y, ray_angle, wall_hit.dist, false);
            if cover_hit.tile == TILE_COVER && cover_hit.dist < wall_hit.dist {
                let c_dist = cover_hit.dist * cos_fix;
                cover_z_buffer[i] = c_dist;
                let full_h = (1.0 / c_dist.max(0.05)) * focal_dist;
                let c_bottom = horizon + full_h * eye_height_factor;
                let c_h = full_h * 0.48;
                let c_top = c_bottom - c_h;

                let mut cu = if cover_hit.side == 0 {
                    cover_hit.hit_y
                } else {
                    cover_hit.hit_x
                };
                cu -= cu.floor();
                let c_shade = (1.0 - (c_dist / 20.0).min(0.55))
                    * if cover_hit.side == 1 { 0.85 } else { 1.0 };
                push_quad(
                    &mut cover_verts,
                    i as f32 * col_w,
                    c_top,
                    col_w,
                    c_h,
                    [cu, 0.0, cu + 0.008, 1.0],
                    [c_shade, c_shade, c_shade, 1.0],
                );
            } else {
                cover_z_buffer[i] = wall_dist;
            }
        }

        // 3. Sort & Prepare 20-Frame Units (with 2D Bone Skinning + Breathing + Coat Sway)
        let mut render_order: Vec<(usize, f32, f32)> = enemies
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, e)| {
                e.screen_bounds = None;
                let dx = e.x - player.x;
                let dy = e.y - player.y;
                let dist = dx.hypot(dy);
                let mut rel = dy.atan2(dx) - player.angle;
                while rel > std::f32::consts::PI {
                    rel -= std::f32::consts::TAU;
                }
                while rel < -std::f32::consts::PI {
                    rel += std::f32::consts::TAU;
                }
                let perp = dist * rel.cos();
                if perp > 0.25 && rel.abs() < base_fov * 0.85 {
                    Some((idx, perp, rel))
                } else {
                    None
                }
            })
            .collect();
        render_order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut unit_draws: Vec<(usize, String, f32, f32)> = Vec::new();
        let mut ubuf_bytes = vec![0u8; self.unit_stride * self.max_units];

        for (slot, (idx, perp_dist, rel_angle)) in render_order.iter().enumerate() {
            if slot >= self.max_units {
                break;
            }
            let e = &mut enemies[*idx];
            let screen_x = w * 0.5 + rel_angle.tan() * focal_dist;
            let full_h = (1.0 / *perp_dist) * focal_dist;
            let sprite_h = full_h * 0.96;
            let sprite_w = sprite_h * (SPRITE_W as f32 / SPRITE_H as f32);
            let ground_y = horizon + full_h * eye_height_factor;

            let (frame_id, flip, bob_y) = e.select_frame(player.x, player.y, time);
            let draw_y = ground_y - sprite_h - (bob_y / SPRITE_H as f32) * sprite_h;
            let draw_x = screen_x - sprite_w * 0.5;

            e.screen_bounds = Some(ScreenBounds {
                x: draw_x,
                y: draw_y,
                w: sprite_w,
                h: sprite_h,
                perp_dist: *perp_dist,
            });

            // Check wall & low-cover occlusion at center column
            let center_col = ((screen_x / col_w) as isize).clamp(0, num_cols as isize - 1) as usize;
            if *perp_dist >= z_buffer[center_col] {
                continue;
            }

            let mut max_vis_y_ratio = 1.0_f32;
            let cover_in_front = cover_z_buffer[center_col];
            if *perp_dist > cover_in_front && cover_in_front < z_buffer[center_col] {
                let c_full_h = (1.0 / cover_in_front) * focal_dist;
                let c_bottom = horizon + c_full_h * eye_height_factor;
                let c_top = c_bottom - c_full_h * 0.48;
                max_vis_y_ratio = ((c_top - draw_y) / sprite_h).clamp(0.0, 1.0);
            }

            // Contact shadow
            push_quad(
                &mut ui_solid_verts,
                screen_x - sprite_w * 0.28,
                ground_y - 4.0,
                sprite_w * 0.56,
                (sprite_h * 0.04).max(4.0),
                [0.0, 0.0, 1.0, 1.0],
                [0.08, 0.09, 0.12, 0.42],
            );

            // Pack 20-frame atlas UV
            let col = (frame_id as u32) % ATLAS_COLS;
            let row = (frame_id as u32) / ATLAS_COLS;
            let u0 = (col * SPRITE_W) as f32 / ATLAS_W as f32;
            let v0 = (row * SPRITE_H) as f32 / ATLAS_H as f32;
            let u1 = ((col + 1) * SPRITE_W) as f32 / ATLAS_W as f32;
            let v1 = ((row + 1) * SPRITE_H) as f32 / ATLAS_H as f32;

            let alive = e.hp > 0;
            let uni = UnitUniform {
                rect: [draw_x, draw_y, sprite_w, sprite_h],
                uv_rect: [u0, v0, u1, v1],
                params: [
                    time + e.phase_offset,
                    if alive { 0.028 } else { 0.0 },
                    if alive { 0.018 } else { 0.0 },
                    (e.hit_flash * 5.0).clamp(0.0, 1.0),
                ],
                clip_info: [if flip { 1.0 } else { 0.0 }, max_vis_y_ratio, 0.0, 0.0],
                bone_rows: e.skeleton.gpu_rows(),
            };
            let offset = slot * self.unit_stride;
            ubuf_bytes[offset..offset + std::mem::size_of::<UnitUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&uni));

            let label = format!(
                "#{} {}{}",
                frame_id,
                FRAME_NAMES[frame_id],
                if flip { " (Flip)" } else { "" }
            );
            unit_draws.push((slot, label, screen_x, draw_y));

            // Enemy HP bar
            if e.hp > 0 && e.hp < e.max_hp {
                let bw = (sprite_w * 0.55).max(30.0);
                let bx = screen_x - bw * 0.5;
                let by = draw_y - 12.0;
                push_quad(&mut ui_solid_verts, bx, by, bw, 5.0, [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.7]);
                push_quad(
                    &mut ui_solid_verts,
                    bx,
                    by,
                    bw * (e.hp as f32 / e.max_hp as f32),
                    5.0,
                    [0.0, 0.0, 1.0, 1.0],
                    [0.90, 0.22, 0.27, 1.0],
                );
            }
        }
        if !unit_draws.is_empty() {
            self.queue.write_buffer(&self.unit_ubuf, 0, &ubuf_bytes);
        }

        // 4. 1st-Person Weapon (driven by 2-Bone Weapon Skeleton)
        let mut weapon_verts: Vec<QuadVertex> = Vec::with_capacity(6);
        let wp_scale = if player.aiming { 0.88 } else { 0.82 };
        let wp_w = 512.0 * wp_scale;
        let wp_h = 340.0 * wp_scale;
        let wp_base_x = if player.aiming { w * 0.44 } else { w * 0.56 } + player.bob_x;
        let wp_base_y = h - wp_h
            + if player.aiming { 24.0 } else { 38.0 }
            + if player.reloading { 110.0 } else { 0.0 }
            + player.bob_y.abs()
            + player.recoil * 28.0;
        let bone_off = weapon_skel.skin_mats[1].transform_point(0.0, 0.0);
        push_quad(
            &mut weapon_verts,
            wp_base_x + bone_off[0] * 12.0,
            wp_base_y + bone_off[1] * 12.0,
            wp_w,
            wp_h,
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );

        // 5. Custom 2D UI (Zero external UI frameworks!)
        // Crosshair
        let cx = w * 0.5;
        let cy = h * 0.5;
        let spread = if player.aiming { 6.0 } else { 14.0 + player.recoil * 16.0 };
        let ch_col = if player.aiming {
            [1.0, 0.75, 0.05, 1.0]
        } else {
            [1.0, 1.0, 1.0, 0.9]
        };
        push_quad(&mut ui_solid_verts, cx - spread - 7.0, cy - 1.0, 7.0, 2.0, [0.0, 0.0, 1.0, 1.0], ch_col);
        push_quad(&mut ui_solid_verts, cx + spread, cy - 1.0, 7.0, 2.0, [0.0, 0.0, 1.0, 1.0], ch_col);
        push_quad(&mut ui_solid_verts, cx - 1.0, cy - spread - 7.0, 2.0, 7.0, [0.0, 0.0, 1.0, 1.0], ch_col);
        push_quad(&mut ui_solid_verts, cx - 1.0, cy + spread, 2.0, 7.0, [0.0, 0.0, 1.0, 1.0], ch_col);

        // Damage vignette
        if player.damage_flash > 0.01 {
            push_quad(
                &mut ui_solid_verts,
                0.0,
                0.0,
                w,
                h,
                [0.0, 0.0, 1.0, 1.0],
                [0.85, 0.1, 0.1, player.damage_flash * 0.4],
            );
        }

        // 2D Minimap
        let tile = 9.0_f32;
        let mx = 14.0_f32;
        let my = 14.0_f32;
        push_quad(
            &mut ui_solid_verts,
            mx - 4.0,
            my - 4.0,
            MAP_W as f32 * tile + 8.0,
            MAP_H as f32 * tile + 8.0,
            [0.0, 0.0, 1.0, 1.0],
            [0.06, 0.09, 0.16, 0.78],
        );
        for gy in 0..MAP_H {
            for gx in 0..MAP_W {
                let t = MAP_DATA[gy][gx];
                if t == TILE_WALL {
                    push_quad(
                        &mut ui_solid_verts,
                        mx + gx as f32 * tile,
                        my + gy as f32 * tile,
                        tile - 1.0,
                        tile - 1.0,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.80, 0.84, 0.88, 1.0],
                    );
                } else if t == TILE_COVER {
                    push_quad(
                        &mut ui_solid_verts,
                        mx + gx as f32 * tile + 1.0,
                        my + gy as f32 * tile + 1.0,
                        tile - 3.0,
                        tile - 3.0,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.85, 0.47, 0.02, 1.0],
                    );
                }
            }
        }
        for e in enemies.iter() {
            if e.hp <= 0 {
                continue;
            }
            let ec = if e.state == EnemyState::Aim || e.state == EnemyState::Shoot {
                [0.94, 0.27, 0.27, 1.0]
            } else {
                [0.22, 0.74, 0.97, 1.0]
            };
            push_quad(&mut ui_solid_verts, mx + e.x * tile - 2.5, my + e.y * tile - 2.5, 5.0, 5.0, [0.0, 0.0, 1.0, 1.0], ec);
        }
        push_quad(&mut ui_solid_verts, mx + player.x * tile - 3.0, my + player.y * tile - 3.0, 6.0, 6.0, [0.0, 0.0, 1.0, 1.0], [0.98, 0.80, 0.08, 1.0]);

        // Top-right HUD Panel (HP, Ammo, Stance, Targets)
        let alive_count = enemies.iter().filter(|e| e.hp > 0).count();
        let hud_w = 290.0_f32;
        let hud_x = w - hud_w - 14.0;
        let hud_y = 14.0;
        push_quad(&mut ui_solid_verts, hud_x, hud_y, hud_w, 72.0, [0.0, 0.0, 1.0, 1.0], [0.06, 0.09, 0.16, 0.80]);
        draw_text(&mut font_verts, hud_x + 10.0, hud_y + 10.0, 2.0, &format!("HP: {}", player.hp), [0.29, 0.87, 0.50, 1.0]);
        let ammo_str = if player.reloading {
            "RELOADING...".to_string()
        } else {
            format!("AMMO: {}/{}", player.ammo, player.reserve_ammo)
        };
        draw_text(&mut font_verts, hud_x + 120.0, hud_y + 10.0, 2.0, &ammo_str, [0.98, 0.80, 0.08, 1.0]);
        let stance_str = if player.crouch_blend > 0.5 { "STANCE: CROUCH" } else { "STANCE: STAND" };
        draw_text(&mut font_verts, hud_x + 10.0, hud_y + 32.0, 2.0, stance_str, [0.22, 0.74, 0.97, 1.0]);
        draw_text(
            &mut font_verts,
            hud_x + 10.0,
            hud_y + 52.0,
            2.0,
            &format!("TARGETS: {} / {}  [V] DEBUG", alive_count, enemies.len()),
            [0.98, 0.44, 0.52, 1.0],
        );

        // Debug labels & 20-frame sprite strip
        if show_debug {
            for (_, label, sx, dy) in &unit_draws {
                let tw = label.len() as f32 * (CELL_W as f32 * 1.5);
                push_quad(&mut ui_solid_verts, sx - tw * 0.5 - 4.0, dy - 26.0, tw + 8.0, 15.0, [0.0, 0.0, 1.0, 1.0], [0.06, 0.09, 0.16, 0.85]);
                draw_text(&mut font_verts, sx - tw * 0.5, dy - 23.0, 1.5, label, [0.22, 0.74, 0.97, 1.0]);
            }

            let thumb_w = 34.0_f32;
            let thumb_h = 51.0_f32;
            let total_w = 20.0 * (thumb_w + 3.0);
            let start_x = ((w - total_w) * 0.5).max(10.0);
            let sy = h - thumb_h - 10.0;
            push_quad(&mut ui_solid_verts, start_x - 6.0, sy - 18.0, total_w + 12.0, thumb_h + 24.0, [0.0, 0.0, 1.0, 1.0], [0.06, 0.09, 0.16, 0.85]);
            draw_text(&mut font_verts, start_x, sy - 14.0, 1.5, "20-FRAME UNIT ATLAS (RUST + WGSL SHADER):", [0.89, 0.91, 0.94, 1.0]);

            for i in 0..20 {
                let tx = start_x + i as f32 * (thumb_w + 3.0);
                let col = (i as u32) % ATLAS_COLS;
                let row = (i as u32) / ATLAS_COLS;
                let u0 = (col * SPRITE_W) as f32 / ATLAS_W as f32;
                let v0 = (row * SPRITE_H) as f32 / ATLAS_H as f32;
                let u1 = ((col + 1) * SPRITE_W) as f32 / ATLAS_W as f32;
                let v1 = ((row + 1) * SPRITE_H) as f32 / ATLAS_H as f32;
                push_quad(&mut ui_solid_verts, tx, sy, thumb_w, thumb_h, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.12]);
                push_quad(&mut strip_verts, tx, sy, thumb_w, thumb_h, [u0, v0, u1, v1], [1.0, 1.0, 1.0, 1.0]);
            }
        }

        // Popups
        for p in popups {
            let tw = p.text.len() as f32 * (CELL_W as f32 * 2.0);
            draw_text(&mut font_verts, p.x - tw * 0.5, p.y, 2.0, &p.text, p.color);
        }

        // Victory / Defeat Banner
        if let Some(msg) = banner_text {
            push_quad(&mut ui_solid_verts, 0.0, h * 0.36, w, 90.0, [0.0, 0.0, 1.0, 1.0], [0.06, 0.09, 0.16, 0.86]);
            let tw = msg.len() as f32 * (CELL_W as f32 * 3.0);
            draw_text(&mut font_verts, (w - tw) * 0.5, h * 0.42, 3.0, msg, [0.98, 0.80, 0.08, 1.0]);
            let sub = "PRESS [ENTER] TO RESTART";
            let sw = sub.len() as f32 * (CELL_W as f32 * 2.0);
            draw_text(&mut font_verts, (w - sw) * 0.5, h * 0.42 + 34.0, 2.0, sub, [0.95, 0.95, 0.95, 1.0]);
        }

        // Upload vertex buffers & execute RenderPass
        let mut all_quads: Vec<QuadVertex> = Vec::new();
        let solid_range = append_range(&mut all_quads, &solid_verts);
        let wall_range = append_range(&mut all_quads, &wall_verts);
        let cover_range = append_range(&mut all_quads, &cover_verts);
        let weapon_range = append_range(&mut all_quads, &weapon_verts);
        let ui_solid_range = append_range(&mut all_quads, &ui_solid_verts);
        let strip_range = append_range(&mut all_quads, &strip_verts);
        let font_range = append_range(&mut all_quads, &font_verts);

        let quad_vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame_quad_vbuf"),
            contents: bytemuck::cast_slice(&all_quads),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.quad_pipeline);
            rpass.set_bind_group(0, &self.screen_bg, &[]);
            rpass.set_vertex_buffer(0, quad_vbuf.slice(..));

            draw_range(&mut rpass, &self.white_bg, solid_range);
            draw_range(&mut rpass, &self.wall_bg, wall_range);
            draw_range(&mut rpass, &self.cover_bg, cover_range);

            // Units via custom WGSL bone+sway+breathing shader
            if !unit_draws.is_empty() {
                rpass.set_pipeline(&self.unit_pipeline);
                rpass.set_bind_group(0, &self.screen_bg, &[]);
                rpass.set_bind_group(1, &self.unit_atlas_bg, &[]);
                rpass.set_vertex_buffer(0, self.unit_vbuf.slice(..));
                rpass.set_index_buffer(self.unit_ibuf.slice(..), wgpu::IndexFormat::Uint16);

                for (slot, _, _, _) in &unit_draws {
                    let dyn_off = (*slot * self.unit_stride) as u32;
                    rpass.set_bind_group(2, &self.unit_ubuf_bg, &[dyn_off]);
                    rpass.draw_indexed(0..self.unit_index_count, 0, 0..1);
                }

                rpass.set_pipeline(&self.quad_pipeline);
                rpass.set_bind_group(0, &self.screen_bg, &[]);
                rpass.set_vertex_buffer(0, quad_vbuf.slice(..));
            }

            draw_range(&mut rpass, &self.weapon_bg, weapon_range);
            draw_range(&mut rpass, &self.white_bg, ui_solid_range);
            draw_range(&mut rpass, &self.unit_atlas_bg, strip_range);
            draw_range(&mut rpass, &self.font_bg, font_range);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

fn append_range(dst: &mut Vec<QuadVertex>, src: &[QuadVertex]) -> std::ops::Range<u32> {
    let start = dst.len() as u32;
    dst.extend_from_slice(src);
    start..(dst.len() as u32)
}

fn draw_range<'a>(
    rpass: &mut wgpu::RenderPass<'a>,
    bg: &'a wgpu::BindGroup,
    range: std::ops::Range<u32>,
) {
    if !range.is_empty() {
        rpass.set_bind_group(1, bg, &[]);
        rpass.draw(range, 0..1);
    }
}

fn push_quad(
    out: &mut Vec<QuadVertex>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    uv: [f32; 4],
    color: [f32; 4],
) {
    let [u0, v0, u1, v1] = uv;
    out.extend_from_slice(&[
        QuadVertex { pos: [x, y], uv: [u0, v0], color },
        QuadVertex { pos: [x + w, y], uv: [u1, v0], color },
        QuadVertex { pos: [x, y + h], uv: [u0, v1], color },
        QuadVertex { pos: [x + w, y], uv: [u1, v0], color },
        QuadVertex { pos: [x + w, y + h], uv: [u1, v1], color },
        QuadVertex { pos: [x, y + h], uv: [u0, v1], color },
    ]);
}

fn push_quad_grad(
    out: &mut Vec<QuadVertex>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    top_col: [f32; 4],
    bot_col: [f32; 4],
) {
    out.extend_from_slice(&[
        QuadVertex { pos: [x, y], uv: [0.0, 0.0], color: top_col },
        QuadVertex { pos: [x + w, y], uv: [1.0, 0.0], color: top_col },
        QuadVertex { pos: [x, y + h], uv: [0.0, 1.0], color: bot_col },
        QuadVertex { pos: [x + w, y], uv: [1.0, 0.0], color: top_col },
        QuadVertex { pos: [x + w, y + h], uv: [1.0, 1.0], color: bot_col },
        QuadVertex { pos: [x, y + h], uv: [0.0, 1.0], color: bot_col },
    ]);
}

fn draw_text(out: &mut Vec<QuadVertex>, x: f32, y: f32, scale: f32, text: &str, color: [f32; 4]) {
    let mut cx = x;
    for b in text.bytes() {
        let uv = glyph_uv(b);
        push_quad(out, cx, y, 5.0 * scale, 7.0 * scale, uv, color);
        cx += CELL_W as f32 * scale;
    }
    let _ = CELL_H;
}
