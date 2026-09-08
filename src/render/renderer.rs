use crate::components::{Camera, Position};
use crate::data::CAMERA_ENTITY_NAME;
use crate::ecs::{Ecs, EntityId};
use crate::render::camera_render_pass::CameraRenderPass;
use crate::render::rect_copy::RectCopyPipeline;
use crate::render::rect_fill::RectFillPipeline;
use crate::world::World;
use crate::{DevUi, MessageWindow, UiData};
use egui::TexturesDelta;
use image::GenericImageView;
use pollster::FutureExt;
use sdl2::video::Window;
use slotmap::SparseSecondaryMap;
use std::collections::HashMap;
use std::format as f;
use std::path::Path;
use tap::{Pipe, TapFallible};
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
    camera_render_passes: SparseSecondaryMap<EntityId, CameraRenderPass>,
    egui_render_pass: egui_wgpu_backend::RenderPass,
    tilesets: HashMap<String, Texture>,
    spritesheets: HashMap<String, Texture>,
    font: FontArc,
    text_brush: TextBrush<FontArc>,
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

        let camera_render_passes: SparseSecondaryMap<EntityId, CameraRenderPass> =
            SparseSecondaryMap::new();

        let egui_render_pass = egui_wgpu_backend::RenderPass::new(&device, surface_format, 1);

        let tilesets = HashMap::new();
        let spritesheets = HashMap::new();

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
            egui_render_pass,
            tilesets,
            spritesheets,
            font,
            text_brush,
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

        // Render camera views
        for (id, camera_comp, position) in ecs.query::<(EntityId, &mut Camera, &Position)>() {
            let Some(camera_render_pass) = self.camera_render_passes.get_mut(id) else {
                continue;
            };

            camera_render_pass.render(
                &mut encoder,
                &self.device,
                &self.queue,
                &self.rect_copy_pipeline,
                &self.tilesets,
                &self.spritesheets,
                id,
                position.0.clone(),
                camera_comp.zoom,
                ecs,
                world,
            );
        }

        self.text_brush.queue(&self.device, &self.queue, [] as [&Section; 0]).unwrap();

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
            if let Some(camera_id) = ecs.query_one_with_name::<EntityId>(CAMERA_ENTITY_NAME)
                // Log error?
                && let Some(camera_render_pass) = self.camera_render_passes.get(camera_id)
            {
                self.rect_copy_pipeline.execute(
                    &mut render_pass,
                    surface_size,
                    &camera_render_pass.texture,
                    0,
                    0,
                    camera_render_pass.texture.size.0,
                    camera_render_pass.texture.size.1,
                    0,
                    0,
                    camera_render_pass.texture.size.0,
                    camera_render_pass.texture.size.1,
                );
            }

            // Draw corner camera
            if let Some(camera_id) = ecs.query_one_with_name::<EntityId>("corner_camera")
                && let Some(camera_render_pass) = self.camera_render_passes.get(camera_id)
            {
                // Draw the border/background
                self.rect_fill_pipeline.execute(
                    &mut render_pass,
                    surface_size,
                    surface_size.0 as i32 - camera_render_pass.texture.size.0 as i32 - 10,
                    surface_size.1 as i32 - camera_render_pass.texture.size.1 as i32 - 10,
                    camera_render_pass.texture.size.0 + 10,
                    camera_render_pass.texture.size.1 + 10,
                    [0., 0., 0., 1.],
                );

                // Draw the camera texture
                self.rect_copy_pipeline.execute(
                    &mut render_pass,
                    surface_size,
                    &camera_render_pass.texture,
                    0,
                    0,
                    camera_render_pass.texture.size.0,
                    camera_render_pass.texture.size.1,
                    surface_size.0 as i32 - camera_render_pass.texture.size.0 as i32,
                    surface_size.1 as i32 - camera_render_pass.texture.size.1 as i32,
                    camera_render_pass.texture.size.0,
                    camera_render_pass.texture.size.1,
                );
            }

            self.draw_message_window(&mut render_pass, surface_size, &ui_data.message_window);
        }

        // Dev UI render pass
        if dev_ui.open
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
        self.text_brush.queue(&self.device, &self.queue, [section]).unwrap();
        self.text_brush.draw(render_pass);
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
    pub fn update_egui_textures_without_rendering(&mut self, textures_delta: TexturesDelta) {
        self.egui_render_pass.add_textures(&self.device, &self.queue, &textures_delta).unwrap();
        self.egui_render_pass.remove_textures(textures_delta).unwrap();
    }
}
