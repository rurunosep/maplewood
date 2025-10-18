use crate::render::renderer::Texture;
use bytemuck::{Pod, Zeroable};
use tap::Pipe;
use wgpu::*;

pub struct RectCopyPipeline {
    pub pipeline: RenderPipeline,
}

impl RectCopyPipeline {
    pub fn new(
        device: &Device,
        surface_format: &TextureFormat,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("rect copy shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/rect_copy_shader.wgsl").into()),
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
            bind_group_layouts: &[&sampler_bind_group_layout, texture_bind_group_layout],
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
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
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

        Self { pipeline }
    }

    pub fn execute(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        sampler_bind_group: &BindGroup,
        src_texture: &Texture,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        dest_x: i32,
        dest_y: i32,
        dest_w: u32,
        dest_h: u32,
    ) {
        let src_tex_w = src_texture.size.0 as f32;
        let src_tex_h = src_texture.size.1 as f32;
        let target_w = render_target_size.0 as f32;
        let target_h = render_target_size.1 as f32;

        let params = RectCopyParams {
            // Map pixel coords to 0to1 tex coords
            src_top: src_y as f32 / src_tex_h,
            src_left: src_x as f32 / src_tex_w,
            src_bottom: (src_y + src_h) as f32 / src_tex_h,
            src_right: (src_x + src_w) as f32 / src_tex_w,
            // Map pixel coords to 0to1, invert Y, and map to -1to1 clip space coords
            dest_top: (dest_y as f32 / target_h).pipe(|x| 1. - x) * 2. - 1.,
            dest_left: (dest_x as f32 / target_w) * 2. - 1.,
            dest_bottom: ((dest_y + dest_h as i32) as f32 / target_h).pipe(|x| 1. - x) * 2. - 1.,
            dest_right: ((dest_x + dest_w as i32) as f32 / target_w) * 2. - 1.,
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, sampler_bind_group, &[]);
        render_pass.set_bind_group(1, &src_texture.bind_group, &[]);
        render_pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[params]));
        render_pass.draw(0..6, 0..1);
    }
}

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
