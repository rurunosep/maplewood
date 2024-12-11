use bytemuck::{Pod, Zeroable};
use image::GenericImageView;
use pollster::FutureExt;
use sdl2::video::Window;
use wgpu::*;

pub struct WgpuRenderData<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    rect_copy_pipeline: RenderPipeline,
    texture_bind_group: BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectCopyParams {
    src_top_left: [f32; 2],
    src_bottom_right: [f32; 2],
    dest_top_left: [f32; 2],
    dest_bottom_right: [f32; 2],
}

pub fn init(window: &Window) -> WgpuRenderData {
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
                required_features: Features::PUSH_CONSTANTS,
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
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: window.size().0,
        height: window.size().1,
        present_mode: PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: CompositeAlphaMode::Auto,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);

    let rect_copy_pipeline = create_rect_copy_pipeline(&device, &surface_format);

    let image = image::open("assets/spritesheets/characters.png").unwrap();
    let texture_size = Extent3d {
        width: image.dimensions().0,
        height: image.dimensions().1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: None,
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        ImageCopyTexture {
            texture: &texture,
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
    let texture_view = texture.create_view(&TextureViewDescriptor::default());
    // Do we really need a separate sampler per texture? I don't think so
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
    let texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &rect_copy_pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&texture_view) },
            BindGroupEntry { binding: 1, resource: BindingResource::Sampler(&sampler) },
        ],
    });

    WgpuRenderData { device, queue, surface, rect_copy_pipeline, texture_bind_group }
}

pub fn render(render_data: &WgpuRenderData) {
    // let start = std::time::Instant::now();

    let output = render_data.surface.get_current_texture().unwrap();
    let view = output.texture.create_view(&TextureViewDescriptor::default());

    let mut encoder =
        render_data.device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for i in 0..1000 {
            let rect_copy_params_data = match i % 2 {
                0 => RectCopyParams {
                    src_top_left: [0.0, 0.0],
                    src_bottom_right: [1.0, 1.0],
                    dest_top_left: [-1.0, 1.0],
                    dest_bottom_right: [-0.5, 0.5],
                },
                _ => RectCopyParams {
                    src_top_left: [0.0, 0.0],
                    src_bottom_right: [1.0, 1.0],
                    dest_top_left: [0.0, 0.0],
                    dest_bottom_right: [0.5, -0.5],
                },
            };

            render_pass.set_pipeline(&render_data.rect_copy_pipeline);
            render_pass.set_bind_group(0, &render_data.texture_bind_group, &[]);
            render_pass.set_push_constants(
                ShaderStages::VERTEX,
                0,
                bytemuck::cast_slice(&[rect_copy_params_data]),
            );
            render_pass.draw(0..6, 0..1);
        }
    }

    render_data.queue.submit([encoder.finish()]);
    output.present();

    // println!("{:.2}%", start.elapsed().as_secs_f64() / (1. / 60.) * 100.);
}

fn create_rect_copy_pipeline(device: &Device, surface_format: &TextureFormat) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(
            std::fs::read_to_string("src/render/shaders/rect_copy_shader.wgsl").unwrap().into(),
        ),
    });

    let texture_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&texture_bind_group_layout],
        push_constant_ranges: &[PushConstantRange {
            stages: ShaderStages::VERTEX,
            // Must have alignment of 4 (this struct happens to require no padding)
            range: 0..std::mem::size_of::<RectCopyParams>() as u32,
        }],
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
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
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    pipeline
}
