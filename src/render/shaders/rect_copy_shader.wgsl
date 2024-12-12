struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) tex_coords: vec2<f32>
}

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

var<push_constant> params: RectCopyParams;

@vertex
fn vertex_main(
  @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
  var tex_coords_array = array(
    vec2f(params.src_left, params.src_bottom),
    vec2f(params.src_right, params.src_top),
    vec2f(params.src_left, params.src_top),
    vec2f(params.src_left, params.src_bottom),
    vec2f(params.src_right, params.src_bottom),
    vec2f(params.src_right, params.src_top),
  );
  var tex_coords = tex_coords_array[vertex_index];

  var position_array = array(
    vec2f(params.dest_left, params.dest_bottom),
    vec2f(params.dest_right, params.dest_top),
    vec2f(params.dest_left, params.dest_top),
    vec2f(params.dest_left, params.dest_bottom),
    vec2f(params.dest_right, params.dest_bottom),
    vec2f(params.dest_right, params.dest_top),
  );
  var position = position_array[vertex_index];

  var output: VertexOutput;
  output.clip_position = vec4f(position, 0.0, 1.0);
  output.tex_coords = tex_coords;

  return output;
}

@group(0) @binding(0) var sampler_: sampler;
@group(1) @binding(0) var texture: texture_2d<f32>;

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(texture, sampler_, input.tex_coords);
}