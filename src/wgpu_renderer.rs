use bytemuck::{Pod, Zeroable};
use pollster::FutureExt;
use sdl2::video::Window;
use wgpu::*;

pub struct WgpuRenderData<'window> {
    device: Device,
    queue: Queue,
    surface: Surface<'window>,
    //
    rect_copy_pipeline: RenderPipeline,
    rect_copy_params_buffer: Buffer,
    rect_copy_bind_group: BindGroup,
    // tiles_pipeline: RenderPipeline,
    // tile_instance_buffer: Buffer,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TileInstance {
    color: [f32; 3],
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
                required_features: Features::empty(),
                required_limits: Limits::default(),
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

    // Rect Copy
    let rect_copy_pipeline = create_rect_copy_pipeline(&device, &surface_format);
    let rect_copy_params_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: std::mem::size_of::<RectCopyParams>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let rect_copy_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &rect_copy_pipeline.get_bind_group_layout(0),
        entries: &[BindGroupEntry {
            binding: 0,
            resource: rect_copy_params_buffer.as_entire_binding(),
        }],
    });

    // Tiles
    // let tiles_pipeline = create_tiles_pipeline(&device, &surface_format);
    // let tile_instance_data = &[
    //     TileInstance { color: [1.0, 0.0, 0.0] },
    //     TileInstance { color: [0.0, 1.0, 0.0] },
    //     TileInstance { color: [0.0, 0.0, 1.0] },
    // ];
    // let tile_instance_buffer = device.create_buffer(&BufferDescriptor {
    //     label: None,
    //     size: (std::mem::size_of::<TileInstance>() * tile_instance_data.len()) as u64,
    //     usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
    //     mapped_at_creation: false,
    // });
    // queue.write_buffer(&tile_instance_buffer, 0, bytemuck::cast_slice(tile_instance_data));

    WgpuRenderData {
        device,
        queue,
        surface,
        rect_copy_pipeline,
        rect_copy_params_buffer,
        rect_copy_bind_group,
        // tiles_pipeline,
        // tile_instance_buffer,
    }
}

pub fn render(render_data: &WgpuRenderData) {
    let output = render_data.surface.get_current_texture().unwrap();
    let view = output.texture.create_view(&TextureViewDescriptor::default());

    let mut encoder =
        render_data.device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
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

        // Tiles
        // render_pass.set_pipeline(&render_data.tiles_pipeline);
        // render_pass.set_vertex_buffer(0, render_data.instance_buffer.slice(..));
        // render_pass.draw(0..6, 0..3);
    }
    render_data.queue.submit([encoder.finish()]);

    rect_copy(&render_data, &view, 1);
    rect_copy(&render_data, &view, 2);

    output.present();
}

// TODO look into push constants
fn rect_copy(render_data: &WgpuRenderData, view: &TextureView, params: i32) {
    let rect_copy_params_data = match params {
        1 => RectCopyParams {
            src_top_left: [0.0, 0.0],
            src_bottom_right: [0.0, 0.0],
            dest_top_left: [-1.0, 1.0],
            dest_bottom_right: [-0.5, 0.5],
        },
        _ => RectCopyParams {
            src_top_left: [0.0, 0.0],
            src_bottom_right: [0.0, 0.0],
            dest_top_left: [0.0, 0.0],
            dest_bottom_right: [0.5, -0.5],
        },
    };
    render_data.queue.write_buffer(
        &render_data.rect_copy_params_buffer,
        0,
        bytemuck::cast_slice(&[rect_copy_params_data]),
    );

    let mut encoder =
        render_data.device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations { load: LoadOp::Load, store: StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&render_data.rect_copy_pipeline);
        render_pass.set_bind_group(0, &render_data.rect_copy_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    render_data.queue.submit([encoder.finish()]);
}

fn create_rect_copy_pipeline(device: &Device, surface_format: &TextureFormat) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(
            std::fs::read_to_string("rect_copy_shader.wgsl").unwrap().into(),
        ),
    });
    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: None,
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

#[allow(dead_code)]
fn create_tiles_pipeline(device: &Device, surface_format: &TextureFormat) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(std::fs::read_to_string("tiles_shader.wgsl").unwrap().into()),
    });
    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<TileInstance>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: &[VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
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
