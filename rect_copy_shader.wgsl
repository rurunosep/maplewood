struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>
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

  return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return vec4f(1.0, 0.0, 0.0, 1.0);
}