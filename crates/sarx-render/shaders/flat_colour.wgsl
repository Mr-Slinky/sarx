// A test shader that draws one polygon in a single colour.

// Places one corner of the polygon, taking its x, y and z from shader location 0
@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Sets w to 1 to leave x, y and z unscaled
    return vec4<f32>(position, 1.0);
}

// Colours every pixel that the polygon covers orange
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.0, 1.0);
}
