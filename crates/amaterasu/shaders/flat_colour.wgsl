// Draws each triangle of a mesh in one colour.

// Places one corner of the triangle, taking its x, y and z from shader location 0
@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Sets w to 1. The GPU divides x, y and z by w, meaning a w of 1 leaves them as they are
    return vec4<f32>(position, 1.0);
}

// Returns orange for every pixel that the triangle covers
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.0, 1.0);
}
