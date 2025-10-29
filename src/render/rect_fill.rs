use bytemuck::{Pod, Zeroable};
use tap::Pipe;
use wgpu::*;

pub struct RectFillPipeline {
    pub pipeline: RenderPipeline,
}

impl RectFillPipeline {
    pub fn new(device: &Device, surface_format: &TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("rect fill shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/rect_fill_shader.wgsl").into()),
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
                    format: *surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self { pipeline }
    }

    pub fn execute(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: [f32; 4],
    ) {
        let target_w = render_target_size.0 as f32;
        let target_h = render_target_size.1 as f32;

        let params = RectFillParams {
            // Map pixel coords to 0to1, invert Y, and map to -1to1 clip space coords
            top: (y as f32 / target_h).pipe(|x| 1. - x) * 2. - 1.,
            left: (x as f32 / target_w) * 2. - 1.,
            bottom: ((y + h as i32) as f32 / target_h).pipe(|x| 1. - x) * 2. - 1.,
            right: ((x + w as i32) as f32 / target_w) * 2. - 1.,
            color,
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[params]));
        render_pass.draw(0..6, 0..1);
    }
}

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
