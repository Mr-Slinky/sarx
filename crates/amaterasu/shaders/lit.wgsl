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

// Matches `ObjectUniforms` in `uniform.rs`
struct Object {
    model: mat4x4<f32>,
}

// Reads the scene uniforms from binding 0 of bind group 0
@group(0) @binding(0) var<uniform> scene: Scene;
// Reads the object uniforms from binding 0 of bind group 1
@group(1) @binding(0) var<uniform> object: Object;

// Receives the two attributes that `Mesh::LAYOUT` describes
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

// Hands the clip-space position to the GPU, and the normal on to the fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Places the vertex in the world, with w set to 1, since it is a point
    let world_position = object.model * vec4<f32>(input.position, 1.0);
    // Places the world point on the screen, through the camera's matrix
    output.clip_position = scene.view_projection * world_position;
    // Rotates the normal with the mesh, with w set to 0, since a direction does not move with a translation
    output.normal = (object.model * vec4<f32>(input.normal, 0.0)).xyz;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Brings the normal back to a length of one, since blending between vertices shortens it
    let normal = normalize(input.normal);
    // Points from the surface towards the light, the opposite of the way the light travels
    let towards_light = normalize(-scene.light_direction.xyz);
    // Measures how directly the surface faces the light, with every surface facing away set to 0
    let diffuse = max(dot(normal, towards_light), 0.0);
    // Adds the ambient share to the light that the surface receives
    let brightness = AMBIENT + (1.0 - AMBIENT) * diffuse;
    // Returns the object's colour at the above brightness, fully opaque
    return vec4<f32>(object.colour.rgb * brightness, 1.0);
}