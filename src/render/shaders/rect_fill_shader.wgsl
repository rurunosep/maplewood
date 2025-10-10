struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) color: vec4<f32>
}

struct RectFillParams {
  top: f32,
  left: f32,
  bottom: f32,
  right: f32,
  color: vec4<f32>
}

var<push_constant> params: RectFillParams;

@vertex
fn vertex_main(
  @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
  var position_array = array(
    vec2f(params.left, params.bottom),
    vec2f(params.right, params.top),
    vec2f(params.left, params.top),
    vec2f(params.left, params.bottom),
    vec2f(params.right, params.bottom),
    vec2f(params.right, params.top),
  );
  var position = position_array[vertex_index];

  var output: VertexOutput;
  output.clip_position = vec4f(position, 0.0, 1.0);
  output.color = params.color;

  return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}