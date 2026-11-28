// Enhanced-mode dust puffs (puffs.rs): one small quad per puff, drawn as a billowing fbm-noise cloud that expands and fades out over its
// life, or (kind 1) as two low sprays running out sideways along the floor. The age comes from the shader clock minus the spawn time
// in the uniform, so the CPU does nothing per frame. Alpha blended, lit only by the colour the CPU picked (the torch or the sun).
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

struct Puff {
    // rgb: the lit dust colour, a: strength
    tint: vec4<f32>,
    // x: the shader clock at spawn, y: life in seconds, z: seed, w: kind (0 cloud, 1 spray)
    p: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> puff: Puff;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.5 * vnoise(p) + 0.3 * vnoise(p * 2.07 + 5.2) + 0.2 * vnoise(p * 4.1 + 9.1);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var dt = globals.time - puff.p.x;
    if dt < -1800.0 {
        dt += 3600.0;
    }
    let a = max(dt, 0.0) / puff.p.y;
    if a >= 1.0 {
        discard;
    }
    let seed = puff.p.z;
    // -1..1, y up; the floor line is at y = -1 (the quad's lower edge)
    let q = vec2<f32>(in.uv.x - 0.5, 0.5 - in.uv.y) * 2.0;
    let ease = 1.0 - (1.0 - a) * (1.0 - a);
    var shape = 0.0;
    var lit = 0.0;
    if puff.p.w < 0.5 {
        // a cloud that swells and rises a little; the noise is stretched and drifts up so it billows
        let c = vec2<f32>(0.0, mix(-0.55, -0.05, ease));
        let rad = mix(0.3, 0.95, ease);
        let d = q - c;
        let n = fbm(d * (2.4 / (0.4 + ease)) + vec2<f32>(seed * 3.1, seed * 1.7 - dt * 0.9));
        shape = 1.0 - length(vec2<f32>(d.x, d.y * 1.25)) / rad + (n - 0.5) * 0.95;
        lit = clamp(dot(normalize(d + vec2<f32>(0.001)), vec2<f32>(-0.45, 0.8)), -1.0, 1.0);
    } else {
        // two sprays leave the middle along the floor, flat and long, torn by the noise
        var best = -1.0;
        for (var s = 0; s < 2; s++) {
            let side = select(-1.0, 1.0, s == 1);
            let c = vec2<f32>(side * mix(0.12, 0.7, ease), -0.72 + 0.18 * ease);
            let rad = vec2<f32>(mix(0.12, 0.34, ease), mix(0.1, 0.24, ease));
            let d = (q - c) / rad;
            let n = fbm(d * 1.6 + vec2<f32>(seed * 2.3 + f32(s) * 7.0, -dt * 0.6));
            best = max(best, 1.0 - length(d) + (n - 0.5) * 1.0);
        }
        shape = best;
        lit = 0.3;
    }
    let dens = smoothstep(-0.08, 0.38, shape);
    // fade in quickly, out slowly; keep clear of the quad's edges
    let fade = smoothstep(0.0, 0.06, a) * pow(1.0 - a, 1.1);
    let edge = smoothstep(1.0, 0.8, abs(q.x)) * smoothstep(1.0, 0.8, abs(q.y));
    let alpha = dens * fade * edge * puff.tint.a;
    if alpha < 0.002 {
        discard;
    }
    return vec4<f32>(puff.tint.rgb * (0.82 + 0.32 * lit), alpha);
}
