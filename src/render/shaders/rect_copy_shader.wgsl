struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) tex_coords: vec2<f32>
}

struct RectCopyParams {
  src_top_left: vec2f,
  src_bottom_right: vec2f,
  dest_top_left: vec2f,
  dest_bottom_right: vec2f,
}

var<push_constant> rect_copy_params: RectCopyParams;

@vertex
fn vertex_main(
  @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
  let src_top_left = rect_copy_params.src_top_left;
  let src_bottom_right = rect_copy_params.src_bottom_right;
  let dest_top_left = rect_copy_params.dest_top_left;
  let dest_bottom_right = rect_copy_params.dest_bottom_right;

  var tex_coords_array = array(
    vec2f(src_top_left.x, src_bottom_right.y),
    vec2f(src_bottom_right.x, src_top_left.y),
    vec2f(src_top_left.x, src_top_left.y),
    vec2f(src_top_left.x, src_bottom_right.y),
    vec2f(src_bottom_right.x, src_bottom_right.y),
    vec2f(src_bottom_right.x, src_top_left.y),
  );
  var tex_coords = tex_coords_array[vertex_index];

  var positions = array(
    vec2f(dest_top_left.x, dest_bottom_right.y),
    vec2f(dest_bottom_right.x, dest_top_left.y),
    vec2f(dest_top_left.x, dest_top_left.y),
    vec2f(dest_top_left.x, dest_bottom_right.y),
    vec2f(dest_bottom_right.x, dest_bottom_right.y),
    vec2f(dest_bottom_right.x, dest_top_left.y),
  );
  var position = positions[vertex_index];

  var output: VertexOutput;
  output.clip_position = vec4f(position, 0.0, 1.0);
  output.tex_coords = tex_coords;

  return output;
}

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var sampler_: sampler;

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(texture, sampler_, input.tex_coords);
}