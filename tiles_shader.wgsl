struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) color: vec4<f32>
}

@vertex
fn vertex_main(
  @builtin(vertex_index) vertex_index: u32,
  @builtin(instance_index) instance_index: u32,
  @location(0) instance_color: vec3<f32>,
) -> VertexOutput {
  // Vertices of a quad
  var positions = array(
    vec2f(-0.5, -0.5),
    vec2f(0.5, 0.5),
    vec2f(-0.5, 0.5),
    vec2f(-0.5, -0.5),
    vec2f(0.5, -0.5),
    vec2f(0.5, 0.5),
  );
  var position = positions[vertex_index];

  // Scale to the size of one tile on screen
  position.x /= (16. / 2);
  position.y /= (12. / 2);

  // Center on the top left of the screen
  position += vec2f(-1.0, 1.0);

  // Shift each instance a bit
  position += vec2f(0.2 * f32(instance_index), -0.2 * f32(instance_index));

  var output: VertexOutput;
  output.clip_position = vec4f(position, 0.0, 1.0);
  output.color = vec4f(instance_color, 1.0);

  return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}