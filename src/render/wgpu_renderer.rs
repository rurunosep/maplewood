use bytemuck::{Pod, Zeroable};
use image::GenericImageView;
use pollster::FutureExt;
use sdl2::video::Window;
use wgpu::*;

pub struct WgpuRenderData<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    surface_size: (u32, u32),
    rect_copy_pipeline: RenderPipeline,
    sampler_bind_group: BindGroup,
    texture: Texture,
}

pub struct Texture {
    // This bind group is only valid for the rect_copy pipeline
    // Where tf do I keep this? Do I keep a bind group for every texture and pipeline
    // combination? And where tf do I store it?
    // TODO pipeline agnostic texture bind group layout?
    bind_group: BindGroup,
    size: (u32, u32),
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

    let rect_copy_pipeline = create_rect_copy_pipeline(&device, &surface_format);

    let image = image::open("assets/spritesheets/characters.png").unwrap();
    let texture_size = Extent3d {
        width: image.dimensions().0,
        height: image.dimensions().1,
        depth_or_array_layers: 1,
    };
    let wgpu_texture = device.create_texture(&TextureDescriptor {
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
    let texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &rect_copy_pipeline.get_bind_group_layout(0),
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&texture_view),
        }],
    });

    let texture = Texture {
        bind_group: texture_bind_group,
        size: (texture_size.width, texture_size.height),
    };

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
    let sampler_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &rect_copy_pipeline.get_bind_group_layout(1),
        entries: &[BindGroupEntry { binding: 0, resource: BindingResource::Sampler(&sampler) }],
    });

    WgpuRenderData {
        device,
        queue,
        surface,
        surface_size,
        rect_copy_pipeline,
        sampler_bind_group,
        texture,
    }
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

        rect_copy(
            &mut render_pass,
            render_data,
            &render_data.texture,
            0,
            0,
            16,
            16,
            0,
            0,
            16,
            16,
        );
    }

    render_data.queue.submit([encoder.finish()]);
    output.present();

    // println!("{:.2}%", start.elapsed().as_secs_f64() / (1. / 60.) * 100.);
}

fn rect_copy(
    render_pass: &mut RenderPass,
    render_data: &WgpuRenderData,
    texture: &Texture,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
    dest_x: u32,
    dest_y: u32,
    dest_w: u32,
    dest_h: u32,
) {
    // Adjust for screen scale
    let dest_x = dest_x * 4;
    let dest_y = dest_y * 4;
    let dest_w = dest_w * 4;
    let dest_h = dest_h * 4;

    // Convert input coords into y-down 0to1 texture coords and y-up -1to1 screen coords
    // TODO clean up all these conversions
    let params = RectCopyParams {
        src_top_left: [
            src_x as f32 / texture.size.0 as f32,
            src_y as f32 / texture.size.1 as f32,
        ],
        src_bottom_right: [
            (src_x + src_w) as f32 / texture.size.0 as f32,
            (src_y + src_h) as f32 / texture.size.1 as f32,
        ],
        dest_top_left: [
            (dest_x as f32 / render_data.surface_size.0 as f32) * 2. - 1.,
            (1. - (dest_y as f32 / render_data.surface_size.1 as f32)) * 2. - 1.,
        ],
        dest_bottom_right: [
            ((dest_x + dest_w) as f32 / render_data.surface_size.0 as f32) * 2. - 1.,
            (1. - ((dest_y + dest_h) as f32 / render_data.surface_size.1 as f32)) * 2. - 1.,
        ],
    };

    render_pass.set_pipeline(&render_data.rect_copy_pipeline);
    render_pass.set_bind_group(0, &render_data.texture.bind_group, &[]);
    render_pass.set_bind_group(1, &render_data.sampler_bind_group, &[]);
    render_pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[params]));
    render_pass.draw(0..6, 0..1);
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
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });

    let sampler_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Sampler(SamplerBindingType::Filtering),
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&texture_bind_group_layout, &sampler_bind_group_layout],
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
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    pipeline
}
