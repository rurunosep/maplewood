use crate::components::{Camera, CameraShake, Position};
use crate::ecs::{Ecs, EntityId};
use crate::math::{PixelUnits, Rect};
use crate::render::camera_render_pass::CameraRenderPass;
use crate::render::rect_copy::RectCopyPipeline;
use crate::render::rect_fill::RectFillPipeline;
use crate::world::World;
use crate::{DevUi, MessageAdvanceCondition, MessageWindow, UiData, misc};
use image::GenericImageView;
use itertools::Itertools;
use pollster::FutureExt;
use sdl2::video::Window;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::format as f;
use std::path::Path;
use tap::{Pipe, TapFallible};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::*;
use wgpu_text::glyph_brush::ab_glyph::FontArc;
use wgpu_text::glyph_brush::{Section, Text};
use wgpu_text::{BrushBuilder, TextBrush};

#[derive(Clone)]
pub struct Texture {
    pub bind_group: BindGroup,
    pub view: TextureView,
    pub size: (u32, u32),
}

pub struct Renderer<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    surface_format: TextureFormat,
    rect_copy_pipeline: RectCopyPipeline,
    rect_fill_pipeline: RectFillPipeline,
    camera_render_passes: HashMap<EntityId, CameraRenderPass>,
    egui_renderer: egui_wgpu::Renderer,
    asset_textures: HashMap<String, Texture>,
    font: FontArc,
    text_brush: TextBrush<FontArc>,
}

impl Renderer<'_> {
    pub fn new(window: &Window) -> Self {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::DEBUG | InstanceFlags::VALIDATION,
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
            display: None,
        });

        // If SDL ever internally recreates the window for whatever reason, the raw window handle
        // will become invalid. Idk if that will ever come up, but it's good to know.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(window.display_handle().unwrap().as_raw()),
                    raw_window_handle: window.window_handle().unwrap().as_raw(),
                })
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
                required_features: Features::IMMEDIATES,
                // Limits should be kept to exactly what we need and no more
                required_limits: Limits { max_immediate_size: 32, ..Default::default() },
                memory_hints: MemoryHints::default(),
                trace: Trace::Off,
                experimental_features: ExperimentalFeatures::disabled(),
            })
            .block_on()
            .unwrap();

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

        let rect_copy_pipeline = RectCopyPipeline::new(&device, &surface_format);
        let rect_fill_pipeline = RectFillPipeline::new(&device, &surface_format);

        let camera_render_passes: HashMap<EntityId, CameraRenderPass> = HashMap::new();

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_format,
            egui_wgpu::RendererOptions {
                msaa_samples: 0,
                depth_stencil_format: None,
                dithering: true,
                predictable_texture_filtering: false,
            },
        );

        let asset_textures = HashMap::new();

        let font_data = std::fs::read("assets/Grand9KPixel.ttf").unwrap();
        let font = FontArc::try_from_vec(font_data.clone()).unwrap();
        let text_brush = BrushBuilder::using_font(font.clone()).build(
            &device,
            surface_size.0,
            surface_size.1,
            surface_format,
        );

        Self {
            device,
            queue,
            surface,
            surface_format,
            rect_copy_pipeline,
            rect_fill_pipeline,
            camera_render_passes,
            egui_renderer,
            asset_textures,
            font,
            text_brush,
        }
    }

    pub fn render(&mut self, world: &World, ecs: &Ecs, ui_data: &UiData, dev_ui: &DevUi) {
        let wgpu::CurrentSurfaceTexture::Success(surface_texture) =
            self.surface.get_current_texture()
        else {
            log::error!(once = true; "Couldn't get the current surface texture to render to");
            return;
        };

        let surface_texture_view =
            surface_texture.texture.create_view(&TextureViewDescriptor::default());
        let surface_size = surface_texture.texture.size().pipe(|s| (s.width, s.height));

        let mut encoder =
            self.device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        // Render camera views
        for (camera_id, camera_component, position, shake) in
            ecs.query::<(EntityId, &Camera, &Position, Option<&CameraShake>)>()
        {
            if camera_component.visible
                && let Some(camera_render_pass) = self.camera_render_passes.get_mut(&camera_id)
            {
                let mut position = position.0.clone();

                // Apply camera shake to position
                if let Some(shake) = shake {
                    let amplitude = shake.amplitude
                        * (1. - shake.start_time.elapsed().div_duration_f64(shake.duration));
                    let y_offset =
                        (shake.start_time.elapsed().as_secs_f64() * shake.frequency * PI * 2.)
                            .sin()
                            * amplitude
                            * -1.;
                    position.map_pos.y += y_offset;
                }

                camera_render_pass.render(
                    &mut encoder,
                    &self.device,
                    &self.queue,
                    &self.rect_copy_pipeline,
                    &self.rect_fill_pipeline,
                    &self.asset_textures,
                    camera_id,
                    position,
                    camera_component.zoom,
                    camera_component.overlay_color,
                    ecs,
                    world,
                );
            };
        }

        // Update dev ui buffers and textures
        if dev_ui.open {
            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &dev_ui.paint_jobs,
                &egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [surface_size.0, surface_size.1],
                    pixels_per_point: dev_ui.ctx.pixels_per_point(),
                },
            );

            for (id, image_delta) in &dev_ui.textures_delta.set {
                self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
            }
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
                multiview_mask: None,
            });

            // Draw cameras to screen
            for (camera_id, camera_component) in ecs
                .query::<(EntityId, &Camera)>()
                .sorted_by(|(_, c1), (_, c2)| c1.z_index.cmp(&c2.z_index))
            {
                if camera_component.visible
                    && let Some(rect_on_screen) = camera_component.rect_on_screen
                    && let Some(camera_render_pass) = self.camera_render_passes.get(&camera_id)
                {
                    if camera_component.border {
                        self.rect_fill_pipeline.execute(
                            &mut render_pass,
                            surface_size,
                            rect_on_screen.x - 10,
                            rect_on_screen.y - 10,
                            rect_on_screen.width as u32 + 20,
                            rect_on_screen.height as u32 + 20,
                            [0., 0., 0., 1.],
                        );
                    }

                    self.rect_copy_pipeline.execute(
                        &mut render_pass,
                        surface_size,
                        &camera_render_pass.texture,
                        0,
                        0,
                        camera_render_pass.texture.size.0,
                        camera_render_pass.texture.size.1,
                        rect_on_screen.x,
                        rect_on_screen.y,
                        rect_on_screen.width as u32,
                        rect_on_screen.height as u32,
                    );
                };
            }

            self.draw_message_window(&mut render_pass, surface_size, &ui_data.message_window);

            // Draw dev ui
            if dev_ui.open {
                self.egui_renderer.render(
                    &mut render_pass.forget_lifetime(),
                    &dev_ui.paint_jobs,
                    &egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [surface_size.0, surface_size.1],
                        pixels_per_point: dev_ui.ctx.pixels_per_point(),
                    },
                );
            }
        }

        // Free old dev ui textures
        for id in &dev_ui.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit([encoder.finish()]);
        surface_texture.present();
    }

    fn draw_message_window<'s, 'rpass>(
        &'s mut self,
        render_pass: &mut RenderPass<'rpass>,
        render_target_size: (u32, u32),
        message_window: &Option<MessageWindow>,
    ) {
        let Some(message_window) = message_window else {
            return;
        };

        // Draw the window itself
        let message_window_nine_slice = NineSlice {
            texture_name: "ui_travelbook.png".to_string(),
            rect_in_texture: Rect::new(160, 336, 64, 32),
            inset_size: 8,
        };
        let rect_on_screen: Rect<u32> = Rect::new(400, 840, 1120, 200);
        draw_nine_slice(
            render_pass,
            &self.rect_copy_pipeline,
            &self.asset_textures,
            render_target_size,
            &message_window_nine_slice,
            rect_on_screen,
            4,
        );

        // Draw the text
        let (visible_chars, hidden_chars) =
            message_window.message.split_at(message_window.num_visible_chars);
        // (split_at panics when splitting within a unicode character of multiple bytes, like "本")
        let visible_text = Text::new(visible_chars).with_scale(48.).with_color([0., 0., 0., 1.]);
        let hidden_text = Text::new(hidden_chars).with_scale(48.).with_color([0., 0., 0., 0.]);
        let section = Section::default()
            .add_text(visible_text)
            .add_text(hidden_text)
            .with_bounds((rect_on_screen.width as f32 - 80., rect_on_screen.height as f32 - 40.))
            .with_screen_position((rect_on_screen.x as f32 + 40., rect_on_screen.y as f32 + 20.));
        // TODO text section queueing should happen outside the render pass
        self.text_brush.queue(&self.device, &self.queue, [section]).unwrap();
        self.text_brush.draw(render_pass);

        // Draw the thingy to show that you can click to advance
        if matches!(message_window.advance_condition, MessageAdvanceCondition::Input) {
            let Some(texture) = self.asset_textures.get("ui_travelbook.png") else {
                log::error!(once = true; "Texture doesn't exist: ui_travelbook.png");
                return;
            };

            let tex_rect: Rect<u32> = Rect::new(48, 496, 16, 16);

            let bob_frequency = 1.5;
            let bob_amplitude = 8.;
            let y_offset =
                (misc::get_seconds_since_start() * bob_frequency * PI * 2.).sin() * bob_amplitude;

            self.rect_copy_pipeline.execute(
                render_pass,
                render_target_size,
                texture,
                tex_rect.x,
                tex_rect.y,
                tex_rect.width,
                tex_rect.height,
                rect_on_screen.right() as i32 - 80,
                rect_on_screen.bottom() as i32 - 80 + y_offset as i32,
                64,
                64,
            );
        }
    }

    pub fn load_asset_textures(&mut self) {
        // Tilesets
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

                self.asset_textures.insert(f!("../assets/tilesets/{file_name}"), texture);

                Some(())
            })
            .for_each(drop);
        }

        // Spritesheets
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

                // TODO store as "spritesheets/{file_name}" or something like that
                self.asset_textures.insert(file_stem, texture);

                Some(())
            })
            .for_each(drop);
        }

        // Ui Spritesheet
        let start = std::time::Instant::now();
        let path = "assets/ui_travelbook.png";
        let texture = self.load_texture(path);
        log::debug!("Loaded {} in {:.2} secs", path, start.elapsed().as_secs_f64());
        self.asset_textures.insert("ui_travelbook.png".to_string(), texture);
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
            layout: &self.rect_copy_pipeline.texture_bind_group_layout,
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

    pub fn create_camera_render_pass(
        &mut self,
        camera_id: EntityId,
        render_target_size: (u32, u32),
    ) {
        let wgpu_texture = self.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: render_target_size.0,
                height: render_target_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.surface_format.clone(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let texture_view = wgpu_texture.create_view(&TextureViewDescriptor::default());
        let camera_texture_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.rect_copy_pipeline.texture_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_view),
            }],
        });
        let texture = Texture {
            bind_group: camera_texture_bind_group,
            view: texture_view,
            size: render_target_size,
        };

        let text_brush = BrushBuilder::using_font(self.font.clone()).build(
            &self.device,
            render_target_size.0,
            render_target_size.1,
            self.surface_format.clone(),
        );

        let camera_render_pass = CameraRenderPass { texture, text_brush };

        self.camera_render_passes.insert(camera_id, camera_render_pass);
    }

    // Part of the hack to make egui properly set initial screen_rect
    pub fn update_egui_textures_without_rendering(&mut self, textures_delta: egui::TexturesDelta) {
        for (id, image_delta) in &textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }
        for id in &textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
    }
}

struct NineSlice {
    texture_name: String,
    rect_in_texture: Rect<u32, PixelUnits>,
    inset_size: u32,
}

// TODO validation or some other fix. currently panics on u32 subtraction with underflow
fn draw_nine_slice<'rpass>(
    render_pass: &mut RenderPass<'rpass>,
    rect_copy_pipeline: &RectCopyPipeline,
    asset_textures: &HashMap<String, Texture>,
    render_target_size: (u32, u32),
    nine_slice: &NineSlice,
    screen_rect: Rect<u32>,
    scale: u32,
) {
    let Some(texture) = asset_textures.get(&nine_slice.texture_name) else {
        log::error!(once = true; "Texture doesn't exist: {}", nine_slice.texture_name);
        return;
    };

    let inset = nine_slice.inset_size;
    let tex_rect = nine_slice.rect_in_texture;

    let tex_slices_and_screen_slices: [(Rect<u32>, Rect<u32>); 9] = [
        // Top left
        (
            Rect::new(tex_rect.left(), tex_rect.top(), inset, inset),
            Rect::new(screen_rect.left(), screen_rect.top(), inset * scale, inset * scale),
        ),
        // Top
        (
            Rect::new(tex_rect.left() + inset, tex_rect.top(), tex_rect.width - inset * 2, inset),
            Rect::new(
                screen_rect.left() + inset * scale,
                screen_rect.top(),
                screen_rect.width - inset * 2 * scale,
                inset * scale,
            ),
        ),
        // Top right
        (
            Rect::new(tex_rect.right() - inset, tex_rect.top(), inset, inset),
            Rect::new(
                screen_rect.right() - inset * scale,
                screen_rect.top(),
                inset * scale,
                inset * scale,
            ),
        ),
        // Left
        (
            Rect::new(tex_rect.left(), tex_rect.top() + inset, inset, tex_rect.height - inset * 2),
            Rect::new(
                screen_rect.left(),
                screen_rect.top() + inset * scale,
                inset * scale,
                screen_rect.height - inset * 2 * scale,
            ),
        ),
        // Middle
        (
            Rect::new(
                tex_rect.left() + inset,
                tex_rect.top() + inset,
                tex_rect.width - inset * 2,
                tex_rect.height - inset * 2,
            ),
            Rect::new(
                screen_rect.left() + inset * scale,
                screen_rect.top() + inset * scale,
                screen_rect.width - inset * 2 * scale,
                screen_rect.height - inset * 2 * scale,
            ),
        ),
        // Right
        (
            Rect::new(
                tex_rect.right() - inset,
                tex_rect.top() + inset,
                inset,
                tex_rect.height - inset * 2,
            ),
            Rect::new(
                screen_rect.right() - inset * scale,
                screen_rect.top() + inset * scale,
                inset * scale,
                screen_rect.height - inset * 2 * scale,
            ),
        ),
        // Bottom left
        (
            Rect::new(tex_rect.left(), tex_rect.bottom() - inset, inset, inset),
            Rect::new(
                screen_rect.left(),
                screen_rect.bottom() - inset * scale,
                inset * scale,
                inset * scale,
            ),
        ),
        // Bottom
        (
            Rect::new(
                tex_rect.left() + inset,
                tex_rect.bottom() - inset,
                tex_rect.width - inset * 2,
                inset,
            ),
            Rect::new(
                screen_rect.left() + inset * scale,
                screen_rect.bottom() - inset * scale,
                screen_rect.width - inset * 2 * scale,
                inset * scale,
            ),
        ),
        // Bottom right
        (
            Rect::new(tex_rect.right() - inset, tex_rect.bottom() - inset, inset, inset),
            Rect::new(
                screen_rect.right() - inset * scale,
                screen_rect.bottom() - inset * scale,
                inset * scale,
                inset * scale,
            ),
        ),
    ];

    for (tex_rect, screen_rect) in tex_slices_and_screen_slices {
        rect_copy_pipeline.execute(
            render_pass,
            render_target_size,
            texture,
            tex_rect.x,
            tex_rect.y,
            tex_rect.width,
            tex_rect.height,
            screen_rect.x as i32,
            screen_rect.y as i32,
            screen_rect.width,
            screen_rect.height,
        );
    }
}
