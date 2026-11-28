// Enhanced-mode atmosphere effects (fx.rs): window light shafts, room dust motes and torch embers. One additive unlit material,
// `fx.kind.x` picks the effect: 0 shaft, 1 dust, 2 embers. Everything animates from `globals.time`; nothing here is per frame CPU work.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;
import bevy_pbr::render::mesh_functions::get_world_from_local;

struct Fx {
    // rgb: the light colour (shaft: the sun's), a: unused
    color: vec4<f32>,
    // x: effect, y z: quad size in world units (embers) / room origin (dust)
    kind: vec4<f32>,
    // dust: xy world position of a torch, z its strength (0 = none)
    torch: array<vec4<f32>, 4>,
    // dust: x = bit row * 10 + col of the room's solid tiles; the room's origin is in kind.yz
    mask: vec4<u32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> fx: Fx;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let a = hash21(p);
    return vec2<f32>(a, hash21(p + a * 19.19 + 7.7));
}

// One layer of sparse motes on a grid of `cells` per world unit: a mote per cell with chance `density`, circling slowly inside
// its cell, twinkling, the whole layer drifting by `drift` (world units per second). Returns 0..1.
fn motes(p: vec2<f32>, cells: f32, drift: vec2<f32>, seed: f32, density: f32, radius: f32) -> f32 {
    let t = globals.time;
    let q = (p + drift * t) * cells;
    let id = floor(q) + seed;
    let f = fract(q);
    let h = hash22(id);
    let ph = t * (0.25 + 0.5 * h.x) + h.y * 6.2831;
    let c = vec2<f32>(0.5) + (h - 0.5) * 0.5 + 0.12 * vec2<f32>(sin(ph), cos(ph * 0.8));
    let alive = step(1.0 - density, hash21(id * 1.37 + 3.1));
    let tw = 0.55 + 0.45 * sin(t * (0.5 + h.y) + h.x * 6.2831);
    return alive * tw * (1.0 - smoothstep(radius * 0.35, radius, length(f - c)));
}

// Three layers (near big and slow, far small and slower) for a feel of depth.
fn air(p: vec2<f32>) -> f32 {
    return motes(p, 4.0, vec2<f32>(0.020, 0.012), 0.0, 0.22, 0.11) * 1.0
        + motes(p, 6.5, vec2<f32>(-0.012, 0.016), 17.0, 0.20, 0.11) * 0.7
        + motes(p, 10.0, vec2<f32>(0.008, 0.010), 41.0, 0.18, 0.13) * 0.45;
}

fn shaft(in: VertexOutput) -> vec3<f32> {
    let t = globals.time;
    let u = in.uv.x;
    let v = in.uv.y; // 0 at the window, 1 at the floor
    // soft sides, a fade-in at the opening, a long fade towards the floor (the pool of light on the floor stays a little)
    let side = smoothstep(0.0, 0.5, u) * smoothstep(1.0, 0.5, u);
    let body = side * side * smoothstep(0.0, 0.2, v) * (1.0 - 0.8 * smoothstep(0.2, 1.0, v));
    // slow stripes along the beam and a gentle overall shimmer
    let stripes = 1.0 + 0.10 * sin(u * 9.0 + t * 0.35 + v * 2.5) + 0.06 * sin(u * 17.0 - t * 0.23);
    let shimmer = 0.92 + 0.08 * sin(t * 0.9 + in.world_position.x * 2.1);
    let dust = air(in.world_position.xy) * smoothstep(0.1, 0.5, side);
    return fx.color.rgb * body * (0.17 * stripes * shimmer + 0.55 * dust);
}

// The room's tile cell of a world position (room.rs layout: column = x, row 0 at the top, row r's slab spans y - SLAB .. + TILE_H).
fn cell(p: vec2<f32>) -> vec2<i32> {
    let l = p - fx.kind.yz;
    return vec2<i32>(i32(floor(l.x)), 2 - i32(floor((l.y + 0.095) / 2.0)));
}

fn solid(c: vec2<i32>) -> bool {
    if c.x < 0 || c.x > 9 || c.y < 0 || c.y > 2 {
        return false;
    }
    return ((fx.mask.x >> u32(c.y * 10 + c.x)) & 1u) != 0u;
}

fn dust(in: VertexOutput) -> vec3<f32> {
    let wp = in.world_position.xy;
    if solid(cell(wp)) {
        discard;
    }
    var lit = 0.0;
    for (var i = 0; i < 4; i++) {
        let tp = fx.torch[i];
        let d = distance(wp, tp.xy);
        if tp.z <= 0.0 || d > 1.6 {
            continue;
        }
        // a short line of sight to the torch: a solid cell on the way (not the torch's own) hides its dust
        let tc = cell(tp.xy);
        var vis = 1.0;
        for (var s = 0; s < 8; s++) {
            let c = cell(mix(wp, tp.xy, (f32(s) + 0.5) / 8.0));
            if any(c != tc) && solid(c) {
                vis = 0.0;
            }
        }
        lit += tp.z * vis * exp(-d * d / (2.0 * 0.7 * 0.7)) * (1.0 - smoothstep(1.0, 1.6, d));
    }
    if lit < 0.02 {
        discard;
    }
    return vec3<f32>(1.0, 0.68, 0.38) * air(wp) * min(lit, 1.0) * 0.28;
}

fn embers(in: VertexOutput) -> vec3<f32> {
    let w = fx.kind.y;
    let h = fx.kind.z;
    let p = vec2<f32>(in.uv.x * w, (1.0 - in.uv.y) * h); // from the bottom left, world units
    let seed = hash21(get_world_from_local(in.instance_index)[3].xy);
    var acc = vec3<f32>(0.0);
    for (var c = 0; c < 4; c++) {
        let k = f32(c) + seed * 37.0;
        let tt = globals.time * (0.28 + 0.22 * hash21(vec2<f32>(k, 1.0))) + hash21(vec2<f32>(k, 2.0));
        let cyc = floor(tt);
        let ph = fract(tt);
        let present = step(0.3, hash21(vec2<f32>(k + 5.0, cyc)));
        let x0 = (f32(c) + 0.2 + 0.6 * hash21(vec2<f32>(k + 9.0, cyc))) * w * 0.25;
        let pos = vec2<f32>(x0 + sin(ph * 9.0 + k) * 0.03 * ph, ph * h * 0.95);
        let size = 0.03 * (1.0 - 0.4 * ph);
        let a = (1.0 - smoothstep(0.0, size, distance(p, pos))) * pow(1.0 - ph, 0.7) * smoothstep(0.0, 0.08, ph) * present;
        acc += mix(vec3<f32>(7.0, 2.1, 0.28), vec3<f32>(1.96, 0.21, 0.03), ph) * a;
    }
    return acc;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var rgb = vec3<f32>(0.0);
    if fx.kind.x < 0.5 {
        rgb = shaft(in);
    } else if fx.kind.x < 1.5 {
        rgb = dust(in);
    } else {
        rgb = embers(in);
    }
    // AlphaMode::Add is the premultiplied blend: rgb + dst * (1 - a), so alpha 0 adds
    return vec4<f32>(rgb, 0.0);
}
