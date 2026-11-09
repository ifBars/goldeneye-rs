struct Camera { view_projection:mat4x4<f32> }
struct Model { matrix:mat4x4<f32> }
@group(0) @binding(0) var<uniform> camera:Camera;
@group(1) @binding(0) var image:texture_2d<f32>;
@group(1) @binding(1) var image_sampler:sampler;
@group(2) @binding(0) var<uniform> model:Model;
struct VertexOutput {
    @builtin(position) position:vec4<f32>,
    @location(0) uv:vec2<f32>,
    @location(1) color:vec4<f32>,
}
@vertex fn vs_main(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) color:vec4<f32>)->VertexOutput {
    var result:VertexOutput;
    result.position=camera.view_projection*model.matrix*vec4<f32>(position,1.0);
    result.uv=uv;
    result.color=color;
    return result;
}
@fragment fn fs_main(input:VertexOutput)->@location(0) vec4<f32> {
    let color=textureSample(image,image_sampler,input.uv)*input.color;
    if color.a<0.01 {discard;}
    return color;
}
