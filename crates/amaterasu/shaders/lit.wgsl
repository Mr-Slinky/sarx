// The share of the full brightness that every surface receives, even one facing away from the light
const AMBIENT: f32 = 0.2;

// Matches `ObjectUniforms` in `uniform.rs`, field for field
struct Object {
    model: mat4x4<f32>,
    colour: vec4<f32>,
}

// Matches `SceneUniforms` in `uniform.rs`, field for field
struct Scene {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
}

// Reads the scene uniforms from binding 0 of bind group 0
@group(0) @binding(0) var<uniform> scene: Scene;
// Reads the object uniforms from binding 0 of bind group 1
@group(1) @binding(0) var<uniform> object: Object;

// How much darker the dark squares of the ground are than the light ones
const CHECKER_SHADE: f32 = 0.85;

// Receives the two attributes that `Vertex::LAYOUT` describes
struct StaticInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

// Hands the clip-space position to the GPU, and the normal and world position on to the
// fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
}

// Places one vertex in the world through `transform`, and then on the screen
fn place_vertex(transform: mat4x4<f32>, position: vec3<f32>, normal: vec3<f32>) -> VertexOutput {
    var output: VertexOutput;
    // Places the vertex in the world, with w set to 1, since it is a point
    let world_position = transform * vec4<f32>(position, 1.0);
    // Places the world point on the screen, through the camera's matrix
    output.clip_position = scene.view_projection * world_position;
    // Rotates the normal with the mesh, with w set to 0, since a direction does not move with a translation
    output.normal = (transform * vec4<f32>(normal, 0.0)).xyz;
    // Passes the world point on, for the ground's checker
    output.world_position = world_position.xyz;
    return output;
}

@vertex
fn vs_static(input: StaticInput) -> VertexOutput {
    // Places the vertex through the object's model matrix alone
    return place_vertex(object.model, input.position, input.normal);
}

// Returns how brightly the light falls on a surface facing along `surface_normal`
fn find_brightness(surface_normal: vec3<f32>) -> f32 {
    // Brings the normal back to a length of one, since blending between vertices shortens it
    let normal = normalize(surface_normal);
    // Points from the surface towards the light, the opposite of the way the light travels
    let towards_light = normalize(-scene.light_direction.xyz);
    // Measures how directly the surface faces the light, with every surface facing away set to 0
    let diffuse = max(dot(normal, towards_light), 0.0);
    // Adds the ambient share to the light that the surface receives
    return AMBIENT + (1.0 - AMBIENT) * diffuse;
}

@fragment
fn fs_lit(input: VertexOutput) -> @location(0) vec4<f32> {
    // Finds how brightly the light falls on this pixel
    let brightness = find_brightness(input.normal);
    // Returns the object's colour at that brightness, fully opaque
    return vec4<f32>(object.colour.rgb * brightness, 1.0);
}

@fragment
fn fs_ground(input: VertexOutput) -> @location(0) vec4<f32> {
    // Finds how brightly the light falls on this pixel
    let brightness = find_brightness(input.normal);
    // Finds the one-unit square that the pixel lies in, by rounding x and z down
    let cell = vec2<i32>(floor(input.world_position.xz));
    // Marks every other square as dark, like the squares of a chessboard
    let is_dark = ((cell.x + cell.y) & 1) == 1;
    // Darkens the dark squares, and leaves the light ones as they are
    let shade = select(1.0, CHECKER_SHADE, is_dark);
    // Returns the ground's colour at that brightness and shade, fully opaque
    return vec4<f32>(object.colour.rgb * brightness * shade, 1.0);
}
