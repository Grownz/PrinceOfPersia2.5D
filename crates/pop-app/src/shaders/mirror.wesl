// The glass over the palace back wall (mirror.rs): the mirror camera has drawn the reflected room from the very same view into an
// image, so the mirror image of what lies behind this fragment is the image's pixel at the fragment's own screen position. Added
// faint and tinted (premultiplied blending with alpha 0) over the lit wall; the depth test already hid it behind everything in front.
import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};

struct MirrorWall {
    // rgb: strength times the marble tint
    tint: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> mir: MirrorWall;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var image: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var samp: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let c = textureSampleLevel(image, samp, uv, 0.0).rgb;
    return vec4<f32>(c * mir.tint.rgb, 0.0);
}
