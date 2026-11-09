struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(perspective, center) uv: vec2<f32>,
    @location(1) @interpolate(perspective, center) color: vec4<f32>,
    @location(2) @interpolate(flat) rgb_scale: f32,
}
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) rgb_scale: f32, @location(4) clip_w: f32) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position * clip_w, 0.0, clip_w);
    out.uv = uv;
    out.color = color;
    out.rgb_scale = rgb_scale;
    return out;
}
@fragment fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let modulated = textureSample(image, image_sampler, in.uv) * in.color;
    return vec4<f32>(clamp(modulated.rgb * in.rgb_scale, vec3<f32>(0.0), vec3<f32>(1.0)), modulated.a);
}
