use crate::components::{Camera, Position, SineOffsetAnimation, SpriteComp};
use crate::data::CAMERA_ENTITY_NAME;
use crate::ecs::Ecs;
use crate::math::{CellPos, CellUnits, MapPos, MapUnits, PixelUnits, Rect, Vec2};
use crate::misc::CELL_SIZE;
use crate::render::rect_copy::RectCopyPipeline;
use crate::render::rect_fill::RectFillPipeline;
use crate::world::{Map, TileLayer, World};
use crate::{DevUi, MessageWindow, UiData};
use egui::TexturesDelta;
use image::GenericImageView;
use itertools::Itertools;
use pollster::FutureExt;
use sdl2::video::Window;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::format as f;
use std::path::Path;
use tap::{Pipe, TapFallible, TapOptional};
use wgpu::*;
use wgpu_text::glyph_brush::ab_glyph::FontVec;
use wgpu_text::glyph_brush::{Section, Text};
use wgpu_text::{BrushBuilder, TextBrush};

// Rename this?
pub struct Texture {
    pub bind_group: BindGroup,
    pub view: TextureView,
    pub size: (u32, u32),
}

pub struct Renderer<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    egui_render_pass: egui_wgpu_backend::RenderPass,
    texture_bind_group_layout: BindGroupLayout,
    rect_copy_pipeline: RectCopyPipeline,
    rect_fill_pipeline: RectFillPipeline,
    sampler_bind_group: BindGroup,
    tilesets: HashMap<String, Texture>,
    spritesheets: HashMap<String, Texture>,
    brush: TextBrush<FontVec>,
}

impl Renderer<'_> {
    pub fn new(window: &Window) -> Self {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::DEBUG | InstanceFlags::VALIDATION,
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
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
            .request_device(&DeviceDescriptor {
                label: None,
                // Push constants are only available on native. Can't target wasm.
                required_features: Features::PUSH_CONSTANTS,
                // Limits should be kept to exactly what we need and no more
                required_limits: Limits { max_push_constant_size: 32, ..Default::default() },
                memory_hints: MemoryHints::default(),
                trace: Trace::Off,
            })
            .block_on()
            .unwrap();
        // For now, keep the default behavior of panicking on uncaptured wgpu errors
        // If I don't want to panic, I can gracefully log them like this:
        // device.on_uncaptured_error(Box::new(|e| log::error!("Wgpu error: {e}")));

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
            RectCopyPipeline::new(&device, &surface_format, &texture_bind_group_layout);
        let rect_fill_pipeline = RectFillPipeline::new(&device, &surface_format);

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
            layout: &rect_copy_pipeline.pipeline.get_bind_group_layout(0),
            entries: &[BindGroupEntry { binding: 0, resource: BindingResource::Sampler(&sampler) }],
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
        dev_ui: &mut DevUi,
    ) {
        let surface_texture = self.surface.get_current_texture().unwrap();
        let surface_texture_view =
            surface_texture.texture.create_view(&TextureViewDescriptor::default());
        let surface_size = surface_texture.texture.size().pipe(|s| (s.width, s.height));

        let mut encoder =
            self.device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        // Does the camera texture have to be recreated every frame? Can I save and reuse it?
        let camera_texture = ecs.query_one_with_name::<&Camera>(CAMERA_ENTITY_NAME).map(|camera| {
            self.prepare_camera_texture(camera.size, surface_texture.texture.format())
        });

        // Camera render pass
        // Render the world as seen by the camera onto a texture to be later rendered onto the
        // surface at the appropriate scale
        if let Some(camera_texture) = &camera_texture {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &camera_texture.view,
                    resolve_target: None,
                    ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.render_camera_view(&mut render_pass, camera_texture.size, world, ecs);
        }

        // Main render pass
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw camera texture to screen
            if let Some(camera_texture) = &camera_texture {
                self.rect_copy_pipeline.execute(
                    &mut render_pass,
                    surface_size,
                    &self.sampler_bind_group,
                    camera_texture,
                    0,
                    0,
                    camera_texture.size.0,
                    camera_texture.size.1,
                    0,
                    0,
                    surface_size.0,
                    surface_size.1,
                );
            }

            self.draw_message_window(&mut render_pass, surface_size, &ui_data.message_window);
        }

        // Dev UI render pass
        if dev_ui.active
            && let Some(full_output) = dev_ui.full_output.take()
        {
            let paint_jobs =
                dev_ui.ctx.tessellate(full_output.shapes, dev_ui.ctx.pixels_per_point());
            let textures_delta = full_output.textures_delta;

            let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
                physical_width: surface_size.0,
                physical_height: surface_size.1,
                scale_factor: dev_ui.ctx.pixels_per_point(),
            };

            self.egui_render_pass.add_textures(&self.device, &self.queue, &textures_delta).unwrap();
            self.egui_render_pass.update_buffers(
                &self.device,
                &self.queue,
                &paint_jobs,
                &screen_descriptor,
            );

            self.egui_render_pass
                .execute(&mut encoder, &surface_texture_view, &paint_jobs, &screen_descriptor, None)
                .unwrap();

            self.egui_render_pass.remove_textures(textures_delta).unwrap();
        }

        self.queue.submit([encoder.finish()]);
        surface_texture.present();
    }

    fn prepare_camera_texture(
        &self,
        camera_size: Vec2<f64, MapUnits>,
        surface_format: TextureFormat,
    ) -> Texture {
        let camera_texture_size =
            ((camera_size.x * CELL_SIZE as f64) as u32, (camera_size.y * CELL_SIZE as f64) as u32);
        let camera_wgpu_texture = self.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: camera_texture_size.0,
                height: camera_texture_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            // Must have format of surface because rect copy pipeline is configured for it
            format: surface_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let camera_texture_view =
            camera_wgpu_texture.create_view(&TextureViewDescriptor::default());
        let camera_texture_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.texture_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&camera_texture_view),
            }],
        });

        Texture {
            bind_group: camera_texture_bind_group,
            view: camera_texture_view,
            size: camera_texture_size,
        }
    }

    fn render_camera_view<'rpass>(
        &'rpass self,
        render_pass: &mut RenderPass<'rpass>,
        render_target_size: (u32, u32),
        world: &World,
        ecs: &Ecs,
    ) {
        if let Some((camera_position, camera_component)) =
            ecs.query_one_with_name::<(&Position, &Camera)>(CAMERA_ENTITY_NAME)
            && let Some(map) = world.maps.get(&camera_position.map).tap_none(
                || log::error!(once = true; "Map doesn't exist: {}", &camera_position.map),
            )
        {
            let camera_rect: Rect<f64, MapUnits> = Rect::new_from_center(
                camera_position.map_pos.x,
                camera_position.map_pos.y,
                camera_component.size.x,
                camera_component.size.y,
            );

            // Draw tile layers below entities
            for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3") {
                self.draw_tile_layer(render_pass, render_target_size, layer, map, camera_rect);
            }

            // Draw entities
            self.draw_entities(render_pass, render_target_size, ecs, map, camera_rect);

            // Draw tile layers above entities
            for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
                self.draw_tile_layer(render_pass, render_target_size, layer, map, camera_rect);
            }
        }
    }

    fn draw_tile_layer(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        layer: &TileLayer,
        map: &Map,
        camera_rect: Rect<f64, MapUnits>,
    ) {
        let Some(tileset) = self.tilesets.get(&layer.tileset_path) else {
            log::error!(once = true; "Tileset doesn't exist: {}", &layer.tileset_path);
            return;
        };

        let tileset_width_in_tiles = tileset.size.0 / CELL_SIZE;

        let map_bounds: Rect<i32, CellUnits> =
            Rect::new(map.offset.x, map.offset.y, map.dimensions.x, map.dimensions.y);
        for col in map_bounds.left()..map_bounds.right() {
            for row in map_bounds.top()..map_bounds.bottom() {
                let cell_pos = CellPos::new(col, row);
                let vec_coords = cell_pos - map.offset;
                let vec_index = vec_coords.y * map.dimensions.x + vec_coords.x;

                if let Some(tile_id) = layer.tile_ids.get(vec_index as usize).expect("") {
                    let top_left_in_viewport = map_pos_to_top_left_in_viewport(
                        cell_pos.to_map_units(),
                        Some(layer.offset),
                        camera_rect,
                    );

                    let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * CELL_SIZE;
                    let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * CELL_SIZE;

                    self.rect_copy_pipeline.execute(
                        render_pass,
                        render_target_size,
                        &self.sampler_bind_group,
                        tileset,
                        tile_x_in_tileset,
                        tile_y_in_tileset,
                        CELL_SIZE,
                        CELL_SIZE,
                        top_left_in_viewport.x,
                        top_left_in_viewport.y,
                        CELL_SIZE,
                        CELL_SIZE,
                    );
                }
            }
        }
    }

    fn draw_entities(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        ecs: &Ecs,
        map: &Map,
        camera_rect: Rect<f64, MapUnits>,
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

            let top_left_in_viewport =
                map_pos_to_top_left_in_viewport(position, Some(sprite.anchor * -1), camera_rect);

            self.rect_copy_pipeline.execute(
                render_pass,
                render_target_size,
                &self.sampler_bind_group,
                spritesheet,
                sprite.rect.left(),
                sprite.rect.top(),
                sprite.rect.width,
                sprite.rect.height,
                top_left_in_viewport.x,
                top_left_in_viewport.y,
                sprite.rect.width,
                sprite.rect.height,
            );
        }
    }

    fn draw_message_window<'rpass>(
        &'rpass mut self,
        render_pass: &mut RenderPass<'rpass>,
        render_target_size: (u32, u32),
        message_window: &Option<MessageWindow>,
    ) {
        let Some(message_window) = message_window else {
            return;
        };

        // Draw the window itself
        self.rect_fill_pipeline.execute(
            render_pass,
            render_target_size,
            40,
            render_target_size.1 as i32 - 240,
            render_target_size.0 - 80,
            200,
            [0.02, 0.02, 0.02, 1.],
        );

        // Draw the text
        let section = Section::default()
            .add_text(
                Text::new(&message_window.message).with_scale(48.).with_color([1., 1., 1., 1.]),
            )
            .with_screen_position((80., render_target_size.1 as f32 - 224.));
        self.brush.queue(&self.device, &self.queue, [section]).unwrap();
        self.brush.draw(render_pass);
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
                }

                let start = std::time::Instant::now();
                let texture = self.load_texture(&path);
                log::debug!(
                    "Loaded {} in {:.2} secs",
                    path.to_string_lossy(),
                    start.elapsed().as_secs_f64()
                );

                self.tilesets.insert(f!("../assets/tilesets/{file_name}"), texture);

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
                }

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
            TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                // What is this?
                aspect: TextureAspect::All,
            },
            &image.to_rgba8(),
            TexelCopyBufferLayout {
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
            view: texture_view,
            size: (texture_size.width, texture_size.height),
        }
    }

    // Part of the hack to make egui properly set initial screen_rect
    pub fn update_egui_textures_without_rendering(&mut self, textures_delta: TexturesDelta) {
        self.egui_render_pass.add_textures(&self.device, &self.queue, &textures_delta).unwrap();
        self.egui_render_pass.remove_textures(textures_delta).unwrap();
    }
}

fn map_pos_to_top_left_in_viewport(
    map_pos: MapPos,
    sprite_offset: Option<Vec2<i32, PixelUnits>>,
    camera_rect: Rect<f64, MapUnits>,
) -> Vec2<i32, PixelUnits> {
    let map_pos_relative_to_camera_top_left = map_pos - camera_rect.top_left();
    let position_in_viewport = map_pos_relative_to_camera_top_left.to_pixel_units();
    let top_left_in_viewport = position_in_viewport + sprite_offset.unwrap_or_default();
    return top_left_in_viewport;
}
