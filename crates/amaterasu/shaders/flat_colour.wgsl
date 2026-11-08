// Draws one polygon in a single colour. The shader exists to test the pipeline.

// Places one corner of the polygon, taking its x, y and z from shader location 0
@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Sets w to 1. The GPU divides x, y and z by w, meaning a w of 1 leaves them as they are
    return vec4<f32>(position, 1.0);
}

// Returns orange for every pixel that the polygon covers
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.0, 1.0);
}
