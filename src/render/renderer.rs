use crate::components::{Position, SineOffsetAnimation, SpriteComp};
use crate::ecs::Ecs;
use crate::misc::{PixelUnits, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use crate::world::{CellPos, Map, MapPos, TileLayer, World};
use crate::{EguiData, UiData};
use bytemuck::{Pod, Zeroable};
use euclid::{Point2D, Rect, Size2D, Vector2D};
use image::GenericImageView;
use itertools::Itertools;
use pollster::FutureExt;
use sdl2::video::Window;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::Path;
use tap::{Pipe, TapFallible, TapOptional};
use wgpu::*;
use wgpu_text::glyph_brush::ab_glyph::FontVec;
use wgpu_text::glyph_brush::{Section, Text};
use wgpu_text::{BrushBuilder, TextBrush};

#[repr(C)]
#[derive(Clone, Copy)]
struct RectCopyParams {
    src_top: f32,
    src_left: f32,
    src_bottom: f32,
    src_right: f32,
    dest_top: f32,
    dest_left: f32,
    dest_bottom: f32,
    dest_right: f32,
}
unsafe impl Pod for RectCopyParams {}
unsafe impl Zeroable for RectCopyParams {}

#[repr(C)]
#[derive(Clone, Copy)]
struct RectFillParams {
    top: f32,
    left: f32,
    bottom: f32,
    right: f32,
    color: [f32; 4],
}
unsafe impl Pod for RectFillParams {}
unsafe impl Zeroable for RectFillParams {}

pub struct Texture {
    bind_group: BindGroup,
    size: (u32, u32),
}

pub struct Renderer<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    surface_size: (u32, u32),
    egui_render_pass: egui_wgpu_backend::RenderPass,
    texture_bind_group_layout: BindGroupLayout,
    rect_copy_pipeline: RenderPipeline,
    rect_fill_pipeline: RenderPipeline,
    sampler_bind_group: BindGroup,
    tilesets: HashMap<String, Texture>,
    spritesheets: HashMap<String, Texture>,
    brush: TextBrush<FontVec>,
}

impl Renderer<'_> {
    pub fn new(window: &Window) -> Self {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::DEBUG | InstanceFlags::VALIDATION,
            dx12_shader_compiler: Dx12Compiler::default(),
            gles_minor_version: Gles3MinorVersion::default(),
        });

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window).unwrap())
                .unwrap()
        };

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .block_on()
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    // Push constants are only available on native. Can't target wasm.
                    required_features: Features::PUSH_CONSTANTS,
                    // Limits should be kept to exactly what we need and no more
                    required_limits: Limits { max_push_constant_size: 32, ..Default::default() },
                    memory_hints: MemoryHints::default(),
                },
                None,
            )
            .block_on()
            .unwrap();
        // For now, keep the default behavior of panicking on uncaptured wgpu errors
        // If I don't want to panic, I can gracefully log them like this:
        // device.on_uncaptured_error(Box::new(|e| log::error!("{e}")));

        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_capabilities.formats[0]);
        let surface_size = window.size();
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: surface_size.0,
            height: surface_size.1,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        // (I can actually reference the egui_wgpu_backend::RenderPass code to see how it
        // structures and solves several problems. It looks pretty informative.)
        let egui_render_pass = egui_wgpu_backend::RenderPass::new(&device, surface_format, 1);

        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        // What is filterable? and what's a filtering sampler?
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let rect_copy_pipeline =
            create_rect_copy_pipeline(&device, &surface_format, &texture_bind_group_layout);
        let rect_fill_pipeline = create_rect_fill_pipeline(&device, &surface_format);

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        let sampler_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &rect_copy_pipeline.get_bind_group_layout(0),
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Sampler(&sampler),
            }],
        });

        let tilesets = HashMap::new();
        let spritesheets = HashMap::new();

        let font_data = std::fs::read("assets/Grand9KPixel.ttf").unwrap();
        let font = FontVec::try_from_vec(font_data).unwrap();
        let brush = BrushBuilder::using_font(font).build(
            &device,
            surface_size.0,
            surface_size.1,
            surface_format,
        );

        Self {
            device,
            queue,
            surface,
            surface_size,
            egui_render_pass,
            texture_bind_group_layout,
            rect_copy_pipeline,
            rect_fill_pipeline,
            sampler_bind_group,
            tilesets,
            spritesheets,
            brush,
        }
    }

    pub fn render(
        &mut self,
        world: &World,
        ecs: &Ecs,
        ui_data: &UiData,
        // &mut cause we need to consume full_output.textures_delta
        egui_data: &mut EguiData,
    ) {
        // let start = std::time::Instant::now();

        let output = self.surface.get_current_texture().unwrap();
        let view = output.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder =
            self.device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        // Prepare the message window text
        if let Some(message_window) = &ui_data.message_window {
            let section = Section::default()
                .add_text(
                    Text::new(&message_window.message)
                        .with_scale(48.)
                        .with_color([1., 1., 1., 1.]),
                )
                .with_screen_position((20. * 4., (16. * 12. - 56.) * 4.));
            self.brush.queue(&self.device, &self.queue, [section]).unwrap();
        }

        // Main render pass
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw tile layers and entities with rect copy
            // (Direct port of the old sdl renderer code)
            self.sdl_renderer_port(&mut render_pass, world, ecs);

            // Draw message window
            if ui_data.message_window.is_some() {
                self.draw_message_window(&mut render_pass);
            }
        }

        // Egui render pass
        // (May also be done in the same render pass with
        // egui_wgpu_backend::RenderPass::execute_with_render_pass)
        if egui_data.active
            && let Some(full_output) = egui_data.full_output.take()
        {
            let paint_jobs =
                egui_data.ctx.tessellate(full_output.shapes, egui_data.ctx.pixels_per_point());
            let textures_delta = full_output.textures_delta;

            let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
                physical_width: self.surface_size.0,
                physical_height: self.surface_size.1,
                scale_factor: egui_data.ctx.pixels_per_point(),
            };

            self.egui_render_pass
                .add_textures(&self.device, &self.queue, &textures_delta)
                .unwrap();
            self.egui_render_pass.update_buffers(
                &self.device,
                &self.queue,
                &paint_jobs,
                &screen_descriptor,
            );

            self.egui_render_pass
                .execute(&mut encoder, &view, &paint_jobs, &screen_descriptor, None)
                .unwrap();

            self.egui_render_pass.remove_textures(textures_delta).unwrap();
        }

        self.queue.submit([encoder.finish()]);
        output.present();

        // println!("{:.2}%", start.elapsed().as_secs_f64() / (1. / 60.) * 100.);
    }

    fn sdl_renderer_port<'rpass>(
        &'rpass self,
        render_pass: &mut RenderPass<'rpass>,
        world: &World,
        ecs: &Ecs,
    ) {
        if let Some(camera_position) = ecs.query_one_with_name::<&Position>("CAMERA")
            && let Some(map) = world.maps.get(&camera_position.map).tap_none(
                || log::error!(once = true; "Map doesn't exist: {}", &camera_position.map),
            )
        {
            let camera_map_pos = camera_position.map_pos;

            // Draw tile layers below entities
            for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3")
            {
                self.draw_tile_layer(render_pass, layer, map, camera_map_pos);
            }

            // Draw entities
            self.draw_entities(render_pass, ecs, map, camera_map_pos);

            // Draw tile layers above entities
            for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
                self.draw_tile_layer(render_pass, layer, map, camera_map_pos);
            }
        }
    }

    fn draw_tile_layer(
        &self,
        render_pass: &mut RenderPass,
        layer: &TileLayer,
        map: &Map,
        camera_map_pos: MapPos,
    ) {
        let Some(tileset) = self.tilesets.get(&layer.tileset_path) else {
            log::error!(once = true; "Tileset doesn't exist: {}", &layer.tileset_path);
            return;
        };

        let tileset_width_in_tiles = tileset.size.0 / 16;

        let map_bounds = Rect::new(map.offset.to_point(), map.dimensions);
        for col in map_bounds.min_x()..map_bounds.max_x() {
            for row in map_bounds.min_y()..map_bounds.max_y() {
                let cell_pos = CellPos::new(col, row);
                let vec_coords = cell_pos - map.offset;
                let vec_index = vec_coords.y * map.dimensions.width + vec_coords.x;

                if let Some(tile_id) = layer.tile_ids.get(vec_index as usize).expect("") {
                    let top_left_in_screen = map_pos_to_screen_top_left(
                        cell_pos.cast().cast_unit(),
                        Some(layer.offset * SCREEN_SCALE as i32),
                        camera_map_pos,
                    );

                    let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * 16;
                    let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * 16;

                    self.rect_copy(
                        render_pass,
                        tileset,
                        tile_x_in_tileset,
                        tile_y_in_tileset,
                        16,
                        16,
                        top_left_in_screen.x,
                        top_left_in_screen.y,
                        16 * 4,
                        16 * 4,
                    );
                }
            }
        }
    }

    fn draw_entities(
        &self,
        render_pass: &mut RenderPass,
        ecs: &Ecs,
        map: &Map,
        camera_map_pos: MapPos,
    ) {
        for (position, sprite_component, sine_offset_animation) in ecs
            .query::<(&Position, &SpriteComp, Option<&SineOffsetAnimation>)>()
            .sorted_by(|(p1, ..), (p2, ..)| p1.map_pos.y.partial_cmp(&p2.map_pos.y).expect(""))
        {
            // Skip entities not on the current map
            if position.map != map.name {
                continue;
            }

            if !sprite_component.visible {
                continue;
            }

            // Choose sprite to draw
            let Some(sprite) =
                sprite_component.forced_sprite.as_ref().or(sprite_component.sprite.as_ref())
            else {
                continue;
            };

            let Some(spritesheet) = self.spritesheets.get(&sprite.spritesheet) else {
                log::error!(once = true; "Spritesheet doesn't exist: {}", &sprite.spritesheet);
                continue;
            };

            // If entity has a SineOffsetAnimation, offset sprite position accordingly
            let mut position = position.map_pos;
            if let Some(soa) = sine_offset_animation {
                let offset = soa.direction
                    * (soa.start_time.elapsed().as_secs_f64() * soa.frequency * (PI * 2.)).sin()
                    * soa.amplitude;
                position += offset;
            }

            let top_left_in_screen = map_pos_to_screen_top_left(
                position,
                Some(sprite.anchor.to_vector() * -1 * SCREEN_SCALE as i32),
                camera_map_pos,
            );

            self.rect_copy(
                render_pass,
                spritesheet,
                sprite.rect.min_x(),
                sprite.rect.min_y(),
                sprite.rect.width(),
                sprite.rect.height(),
                top_left_in_screen.x,
                top_left_in_screen.y,
                sprite.rect.width() * 4,
                sprite.rect.height() * 4,
            );
        }
    }

    fn draw_message_window<'rpass>(&'rpass self, render_pass: &mut RenderPass<'rpass>) {
        // Draw the window itself
        self.rect_fill(
            render_pass,
            10 * 4,
            (16 * 12 - 60) * 4,
            (16 * 16 - 20) * 4,
            50 * 4,
            [0.02, 0.02, 0.02, 1.],
        );

        // Draw the prepared text
        self.brush.draw(render_pass);
    }

    fn rect_copy(
        &self,
        render_pass: &mut RenderPass,
        texture: &Texture,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        dest_x: i32,
        dest_y: i32,
        dest_w: u32,
        dest_h: u32,
    ) {
        // TODO adjust for screen scale inside rect_copy?

        let tex_w = texture.size.0 as f32;
        let tex_h = texture.size.1 as f32;
        let screen_w = self.surface_size.0 as f32;
        let screen_h = self.surface_size.1 as f32;

        let params = RectCopyParams {
            // Map pixel coords to 0to1 tex coords
            src_top: src_y as f32 / tex_h,
            src_left: src_x as f32 / tex_w,
            src_bottom: (src_y + src_h) as f32 / tex_h,
            src_right: (src_x + src_w) as f32 / tex_w,
            // Map pixel coords to 0to1, invert Y, and map to -1to1 clip space coords
            dest_top: (dest_y as f32 / screen_h).pipe(|x| 1. - x) * 2. - 1.,
            dest_left: (dest_x as f32 / screen_w) * 2. - 1.,
            dest_bottom: ((dest_y + dest_h as i32) as f32 / screen_h).pipe(|x| 1. - x) * 2. - 1.,
            dest_right: ((dest_x + dest_w as i32) as f32 / screen_w) * 2. - 1.,
        };

        render_pass.set_pipeline(&self.rect_copy_pipeline);
        render_pass.set_bind_group(0, &self.sampler_bind_group, &[]);
        render_pass.set_bind_group(1, &texture.bind_group, &[]);
        render_pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[params]));
        render_pass.draw(0..6, 0..1);
    }

    fn rect_fill(
        &self,
        render_pass: &mut RenderPass,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: [f32; 4],
    ) {
        let screen_w = self.surface_size.0 as f32;
        let screen_h = self.surface_size.1 as f32;

        let params = RectFillParams {
            // Map pixel coords to 0to1, invert Y, and map to -1to1 clip space coords
            top: (y as f32 / screen_h).pipe(|x| 1. - x) * 2. - 1.,
            left: (x as f32 / screen_w) * 2. - 1.,
            bottom: ((y + h as i32) as f32 / screen_h).pipe(|x| 1. - x) * 2. - 1.,
            right: ((x + w as i32) as f32 / screen_w) * 2. - 1.,
            color,
        };

        render_pass.set_pipeline(&self.rect_fill_pipeline);
        render_pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[params]));
        render_pass.draw(0..6, 0..1);
    }

    pub fn load_tilesets(&mut self) {
        if let Ok(dir) = std::fs::read_dir("assets/tilesets/")
            .tap_err(|_| log::error!("Couldn't open assets/tilesets/"))
        {
            // (map and drop so that closure can return Option so that we can use ? throughout)
            dir.map(|entry| -> Option<()> {
                let path = entry.ok()?.path();
                let file_name = path.file_name()?.to_str()?.to_string();
                let file_extension = path.extension()?;

                if file_extension != "png" {
                    return None;
                };

                let start = std::time::Instant::now();
                let texture = self.load_texture(&path);
                log::debug!(
                    "Loaded {} in {:.2} secs",
                    path.to_string_lossy(),
                    start.elapsed().as_secs_f64()
                );

                self.tilesets.insert(format!("../assets/tilesets/{}", file_name), texture);

                Some(())
            })
            .for_each(drop);
        }
    }

    pub fn load_spritesheets(&mut self) {
        if let Ok(dir) = std::fs::read_dir("assets/spritesheets/")
            .tap_err(|_| log::error!("Couldn't open assets/spritesheets/"))
        {
            // (map and drop so that closure can return Option so that we can use ? throughout)
            dir.map(|entry| -> Option<()> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();
                let file_extension = path.extension()?;

                if file_extension != "png" {
                    return None;
                };

                let start = std::time::Instant::now();
                let texture = self.load_texture(&path);
                log::debug!(
                    "Loaded {} in {:.2} secs",
                    path.to_string_lossy(),
                    start.elapsed().as_secs_f64()
                );

                self.spritesheets.insert(file_stem, texture);

                Some(())
            })
            .for_each(drop);
        }
    }

    fn load_texture<P>(&self, path: P) -> Texture
    where
        P: AsRef<Path>,
    {
        let image = image::open(path.as_ref()).unwrap();

        let texture_size = Extent3d {
            width: image.dimensions().0,
            height: image.dimensions().1,
            depth_or_array_layers: 1,
        };

        let wgpu_texture = self.device.create_texture(&TextureDescriptor {
            label: None,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            ImageCopyTexture {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                // What is this?
                aspect: TextureAspect::All,
            },
            &image.to_rgba8(),
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(image.dimensions().0 * 4),
                rows_per_image: Some(image.dimensions().1),
            },
            texture_size,
        );

        let texture_view = wgpu_texture.create_view(&TextureViewDescriptor::default());

        let texture_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.texture_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_view),
            }],
        });

        Texture {
            bind_group: texture_bind_group,
            size: (texture_size.width, texture_size.height),
        }
    }
}

fn create_rect_copy_pipeline(
    device: &Device,
    surface_format: &TextureFormat,
    texture_bind_group_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("rect copy shader"),
        source: ShaderSource::Wgsl(
            std::fs::read_to_string("src/render/shaders/rect_copy_shader.wgsl").unwrap().into(),
        ),
    });

    let sampler_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("sampler bind group layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Sampler(SamplerBindingType::Filtering),
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("rect copy pipeline layout"),
        bind_group_layouts: &[&sampler_bind_group_layout, &texture_bind_group_layout],
        push_constant_ranges: &[PushConstantRange {
            stages: ShaderStages::VERTEX,
            // Must have alignment of 4 (this struct happens to require no padding)
            range: 0..std::mem::size_of::<RectCopyParams>() as u32,
        }],
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("rect copy pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format.clone(),
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    pipeline
}

fn create_rect_fill_pipeline(device: &Device, surface_format: &TextureFormat) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("rect fill shader"),
        source: ShaderSource::Wgsl(
            std::fs::read_to_string("src/render/shaders/rect_fill_shader.wgsl").unwrap().into(),
        ),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("rect fill pipeline layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[PushConstantRange {
            stages: ShaderStages::VERTEX,
            // Must have alignment of 4 (this struct happens to require no padding)
            range: 0..std::mem::size_of::<RectFillParams>() as u32,
        }],
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("rect fill pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format.clone(),
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    pipeline
}

fn map_pos_to_screen_top_left(
    map_pos: MapPos,
    pixel_offset: Option<Vector2D<i32, PixelUnits>>,
    camera_map_pos: MapPos,
) -> Point2D<i32, PixelUnits> {
    let viewport_size_in_map = Size2D::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let viewport_map_offset = (camera_map_pos - viewport_size_in_map / 2.0).to_vector();
    let position_in_viewport = map_pos - viewport_map_offset;
    let position_on_screen =
        (position_in_viewport * (TILE_SIZE * SCREEN_SCALE) as f64).cast().cast_unit();
    let top_left_in_screen = position_on_screen + pixel_offset.unwrap_or_default().cast_unit();

    return top_left_in_screen;
}
