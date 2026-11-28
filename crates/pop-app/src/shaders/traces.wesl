// Enhanced-mode traces (traces.rs): blood stains and rubble piles lying flat on a floor. A multiply decal like the weathering: the
// output is the factor the lit scene is multiplied with (AlphaMode::Multiply), so the traces take the room's light. `tr.kind.x` picks
// the trace: 0 pool, 1 splatter, 2 rubble. Pure functions of the quad's uv, the seed and the age: no CPU work per frame.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

struct Trace {
    // x: trace, y: seed, z: age in game seconds when the material was made, w: the shader clock then
    kind: vec4<f32>,
    // xy: the quad's size in world units (x across, y in depth), z: the stain's final radius
    size: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> tr: Trace;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let a = hash21(p);
    return vec2<f32>(a, hash21(p + a * 19.19 + 7.7));
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.55 * vnoise(p) + 0.3 * vnoise(p * 2.1 + 5.2) + 0.15 * vnoise(p * 4.3 + 9.1);
}

// Age of the stain in game seconds (the clock wraps once an hour).
fn age() -> f32 {
    var dt = globals.time - tr.kind.w;
    if dt < -1800.0 {
        dt += 3600.0;
    }
    return tr.kind.z + max(dt, 0.0);
}

// The stain's coverage 0..1 at `p` (world units from the quad's centre, y towards the front) and its colour factor.
fn stain(p: vec2<f32>) -> vec4<f32> {
    let seed = tr.kind.y;
    let a = age();
    let r0 = tr.size.z;
    var cov = 0.0;
    if tr.kind.x < 0.5 {
        // a pool grows quickly, then slowly (the same curve as the old disc), with an irregular edge
        let g = 1.0 - exp(-a / 2.5);
        let q = vec2<f32>(p.x, p.y / 0.7) / max(r0 * g, 0.001);
        let ang = atan2(q.y, q.x);
        let n = fbm(vec2<f32>(cos(ang), sin(ang)) * 1.7 + seed * 1.713);
        let edge = 0.8 + 0.5 * (n - 0.5) + 0.1 * sin(ang * 3.0 + seed * 5.0) + 0.06 * sin(ang * 5.0 + seed * 3.0);
        cov = 1.0 - smoothstep(edge - 0.1, edge, length(q));
    } else {
        // a splatter: a blob stretched along the spray and a few droplets around it
        let g = 1.0 - exp(-a / 0.2);
        let dir = select(-1.0, 1.0, hash21(vec2<f32>(seed, 3.0)) > 0.5);
        let q = p / (max(r0 * g, 0.001) * vec2<f32>(1.5, 0.75));
        let n = fbm(q * 1.5 + seed);
        cov = 1.0 - smoothstep(0.55 + 0.35 * n, 0.95 + 0.35 * n, length(q));
        for (var i = 0; i < 6; i++) {
            let h = hash22(vec2<f32>(seed + 1.0, f32(i)));
            let d = r0 * (1.3 + 2.0 * h.y) * g;
            let ang = (h.x - 0.5) * 2.2;
            let c = vec2<f32>(dir * cos(ang) * d, sin(ang) * d * 0.55);
            let rad = r0 * (0.13 + 0.22 * hash21(vec2<f32>(seed + 9.0, f32(i)))) * g;
            cov = max(cov, 1.0 - smoothstep(rad * 0.6, rad, length(p - c)));
        }
    }
    let dry = clamp((a - 8.0) / 142.0, 0.0, 1.0);
    let mottle = 0.82 + 0.36 * vnoise(p * 17.0 + seed);
    // wet: deep red that darkens the stone; dry: brown, less covering
    let wet = vec3<f32>(0.46, 0.035, 0.05);
    let dried = vec3<f32>(0.40, 0.19, 0.15);
    let col = mix(wet, dried, dry) * mottle;
    return vec4<f32>(col, cov * (0.97 - 0.12 * dry));
}

// A heap of small stones: a darker dust patch, then pebbles on a grid (one per cell with some chance, only near the heap's middle),
// each with a soft shadow to the lower right and a stone tone; two sizes.
fn rubble(p: vec2<f32>) -> vec3<f32> {
    let seed = tr.kind.y;
    let heap = 1.0 - smoothstep(0.3, 0.95, length(p / (tr.size.xy * 0.5)) + 0.25 * (vnoise(p * 6.0 + seed) - 0.5));
    var f = 1.0 - 0.22 * heap;
    for (var layer = 0; layer < 2; layer++) {
        let cells = select(6.5, 10.5, layer == 1);
        let q = p * cells + seed * 3.7 + f32(layer) * 11.0;
        let id = floor(q);
        let h = hash22(id);
        let pos = vec2<f32>(0.3 + 0.4 * h.x, 0.3 + 0.4 * h.y);
        let rad = (0.2 + 0.14 * hash21(id + 4.0)) * select(1.0, 0.85, layer == 1);
        let alive = step(0.55 - 0.3 * heap * select(1.0, 0.8, layer == 1), heap * hash21(id + 9.0) * 1.2);
        let d = fract(q) - pos;
        let shadow = 1.0 - smoothstep(rad * 0.6, rad * 1.25, length(d - vec2<f32>(0.1, 0.09)));
        let body = 1.0 - smoothstep(rad * 0.8, rad, length(d * vec2<f32>(1.0, 1.2)));
        let tone = 0.68 + 0.3 * hash21(id + 21.0);
        // the lit upper left edge of a stone is brighter
        let lit = 1.0 - 0.12 * clamp(dot(normalize(d + vec2<f32>(0.001)), vec2<f32>(0.7, 0.7)), -1.0, 1.0);
        f = f * (1.0 - alive * 0.45 * shadow * (1.0 - body)) ;
        f = mix(f, tone * lit, alive * body);
    }
    return vec3<f32>(f * 0.98, f * 0.97, f * 0.95);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = (in.uv - vec2<f32>(0.5)) * tr.size.xy;
    if tr.kind.x < 1.5 {
        let s = stain(p);
        if s.w < 0.003 {
            discard;
        }
        return vec4<f32>(mix(vec3<f32>(1.0), s.rgb, s.w), 1.0);
    }
    return vec4<f32>(rubble(p), 1.0);
}
