// Enhanced-mode weathering (wear.rs): the same stone, aged differently. A multiply decal over the wall faces, the floor tops and the
// back wall of a room: a per-tile tone and tint jitter with a slow drift across the room, and in the dungeon moss and damp from the
// floor line and cobwebs in the corners; in both: grime at the tile bottoms, streaks, hairline cracks and the soot above a torch.
// Everything is a pure function of the world position and the room's seed: the same room looks the same every time. The output
// is the factor the lit scene is multiplied with (AlphaMode::Multiply: rgb * dst, alpha 1).
import bevy_pbr::render::forward_io::VertexOutput;

struct Wear {
    // x: surface (0 wall face, 1 floor top, 2 back wall), y: 1 = palace, z: the room's seed
    kind: vec4<f32>,
    // xy: the room's origin in the world
    org: vec4<f32>,
    // x: bit row * 10 + col of the wall blocks, y: of the floors that get a decal
    mask: vec4<u32>,
    // room xy of a torch flame, z its strength (0 = none)
    torch: array<vec4<f32>, 4>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> wear: Wear;

// the floor slab's thickness: a tile's band runs from SLAB below its floor line to 2 above that
const SLAB = 0.0952;
const MOSS = vec3<f32>(0.42, 0.62, 0.34);

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

fn solid(c: vec2<i32>) -> bool {
    if c.x < 0 || c.x > 9 || c.y < 0 || c.y > 2 {
        return false;
    }
    return ((wear.mask.x >> u32(c.y * 10 + c.x)) & 1u) != 0u;
}

fn floored(c: vec2<i32>) -> bool {
    if c.x < 0 || c.x > 9 || c.y < 0 || c.y > 2 {
        return false;
    }
    return ((wear.mask.y >> u32(c.y * 10 + c.x)) & 1u) != 0u;
}

// Tone and tint of a tile (the same everywhere in it) plus a slow drift and stone-sized patches across tiles; `amp` ~ 0.1 is a
// few percent either way.
fn tone(p: vec2<f32>, id: vec2<f32>, amp: f32) -> vec3<f32> {
    // smooth value noise at about tile size instead of a constant per tile: no step at the column borders
    let s = wear.kind.z;
    let h = vec2<f32>(vnoise(p * vec2<f32>(1.0, 0.5) + s * 7.0), vnoise(p * vec2<f32>(1.0, 0.5) + s * 13.0 + 40.0));
    let b = 1.0 + amp * ((h.x - 0.5) * 1.6 + (fbm(p * 0.5 + s) - 0.5) * 1.1 + (vnoise(p * 2.3 + s * 3.1) - 0.5) * 0.6);
    let t = (h.y - 0.5) * 0.56 * amp;
    return vec3<f32>(b + t, b, b - t);
}

fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

fn heading(a: f32) -> vec2<f32> {
    return vec2<f32>(cos(a), sin(a));
}

// A crack in a tile of `size` (q in the tile's units): a jagged random walk (the stem) with branches off it and sub-branches off those,
// each generation thinner, all from the tile's own hash, in `chance` of the tiles; `px` = the size of a pixel in q (anti-aliasing).
// 0..1 coverage; it ends at the tile's edge.
fn crack(q: vec2<f32>, size: vec2<f32>, id: vec2<f32>, chance: f32, px: f32) -> f32 {
    let h = hash22(id + 41.0);
    if h.x > chance {
        return 0.0;
    }
    var d0 = 1e3;
    var d1 = 1e3;
    var d2 = 1e3;
    var ang = hash21(id + 3.3) * 6.2832;
    var cur = vec2<f32>(0.15 + 0.7 * h.y, (0.2 + 0.6 * hash21(id + 8.8)) * size.y);
    let sl = (0.6 + 0.8 * hash21(id + 5.5)) / 6.0;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let r = hash22(id + vec2<f32>(fi * 7.1, 13.0));
        ang += (r.x - 0.5) * 1.5;
        let nx = cur + heading(ang) * sl;
        d0 = min(d0, seg(q, cur, nx));
        if i >= 1 && r.y < 0.5 {
            var bp = nx;
            var ba = ang + select(-1.0, 1.0, r.y < 0.25) * (0.5 + 0.7 * hash21(id + vec2<f32>(fi, 21.0)));
            for (var j = 0; j < 3; j++) {
                let rb = hash22(id + vec2<f32>(fi * 3.7 + f32(j) * 1.9, 31.0));
                ba += (rb.x - 0.5) * 1.4;
                let bn = bp + heading(ba) * sl * 0.8;
                d1 = min(d1, seg(q, bp, bn));
                if j >= 1 && rb.y < 0.4 {
                    var cp = bn;
                    var ca = ba + select(-1.0, 1.0, rb.y < 0.2) * (0.5 + 0.6 * rb.x);
                    for (var k = 0; k < 2; k++) {
                        let rc = hash22(id + vec2<f32>(fi * 5.3 + f32(j) * 2.9 + f32(k) * 1.3, 47.0));
                        ca += (rc.x - 0.5) * 1.4;
                        let cn = cp + heading(ca) * sl * 0.6;
                        d2 = min(d2, seg(q, cp, cn));
                        cp = cn;
                    }
                }
                bp = bn;
            }
        }
        cur = nx;
    }
    let edge = min(min(q.x, size.x - q.x), min(q.y, size.y - q.y));
    // half widths 0.006 / 0.0045 / 0.0035 (never under half a pixel), edge pixels blended
    let cov = max(max(clamp((max(0.006, 0.5 * px) - d0) / px + 0.5, 0.0, 1.0), clamp((max(0.0045, 0.5 * px) - d1) / px + 0.5, 0.0, 1.0)), clamp((max(0.0035, 0.5 * px) - d2) / px + 0.5, 0.0, 1.0));
    return cov * smoothstep(0.0, 0.04, edge);
}

// A dark streak running down from the top of a tile (u across 0..1, d the distance below the top in tile heights), in
// `chance` of the tiles; fades towards its lower end.
fn streak(u: f32, d: f32, id: vec2<f32>, chance: f32, width: f32) -> f32 {
    let h = hash22(id + 11.0);
    if h.x > chance {
        return 0.0;
    }
    let len = 0.25 + 0.7 * h.y;
    let uc = 0.12 + 0.76 * hash21(id + 3.0) + 0.02 * sin(d * 14.0 + h.y * 9.0);
    let x = (u - uc) / width;
    return exp(-x * x) * (1.0 - smoothstep(0.0, len, d)) * (0.55 + 0.45 * vnoise(vec2<f32>(u * 40.0, d * 9.0 + h.y * 30.0)));
}

// Moss as a multiplier: `m` 0..1 how much.
fn moss(m: f32) -> vec3<f32> {
    return mix(vec3<f32>(1.0), MOSS, clamp(m * 1.2, 0.0, 1.0));
}

// A cobweb in the corner at (cx, ct): coverage 0..1 (`dir` -1 the wall is to the left of the corner, 1 to the right).
fn web(p: vec2<f32>, corner: vec2<f32>, dir: f32) -> f32 {
    let a = (p.x - corner.x) * -dir;
    let b = corner.y - p.y;
    if a < 0.0 || b < 0.0 {
        return 0.0;
    }
    let r = length(vec2<f32>(a, b));
    if a + b > 0.62 {
        return 0.0;
    }
    let ang = atan2(b, a);
    // four spokes and sagging rings between them
    let sp = abs(fract(ang / 0.3927 + 0.5) - 0.5) * 0.3927 * r;
    let spokes = 1.0 - smoothstep(0.003, 0.009, sp);
    let ring = abs(fract(r / 0.11 + 0.18 * sin(ang * 8.0)) - 0.5) * 0.11;
    let rings = 1.0 - smoothstep(0.002, 0.007, ring);
    return max(spokes, rings * step(0.07, r)) * (1.0 - smoothstep(0.4, 0.62, a + b));
}

fn wall_face(p: vec2<f32>, c: vec2<i32>, ty: f32, px: f32) -> vec3<f32> {
    let dungeon = wear.kind.y < 0.5;
    let id = vec2<f32>(c) + vec2<f32>(wear.kind.z, wear.kind.z * 0.37);
    let tx = fract(p.x);
    var f = tone(p, id, 0.15);
    // grime where the block meets the ground and under the ceiling
    f *= 1.0 - 0.14 * (1.0 - smoothstep(0.0, 0.4, ty));
    f *= 1.0 - 0.07 * smoothstep(1.7, 2.0, ty);
    // moss grows from the floor line on a base block, ragged and in patches
    // (a smooth gate along x, not a per-tile coin: no hard end at a column border)
    if dungeon && !solid(c + vec2<i32>(0, 1)) {
        let gate = smoothstep(0.4, 0.6, fbm(vec2<f32>(p.x * 0.7, wear.kind.z + 2.0)));
        let reach = 0.1 + 0.36 * fbm(vec2<f32>(p.x * 2.6, wear.kind.z));
        let m = (1.0 - smoothstep(0.0, reach, ty + (vnoise(p * 14.0) - 0.5) * 0.14)) * gate;
        f *= moss(m * 0.8) * (1.0 - 0.15 * m);
    }
    f *= 1.0 - 0.16 * streak(tx, (2.0 - ty) * 0.5, id, 0.4, 0.04);
    // and a few patches on the face itself
    if dungeon {
        let gate = smoothstep(0.5, 0.75, vnoise(p * 0.6 + wear.kind.z * 2.0));
        let m = smoothstep(0.55, 0.8, fbm(p * 2.2 + wear.kind.z)) * gate * (0.6 + 0.4 * vnoise(p * 14.0));
        f *= moss(m * 0.6) * (1.0 - 0.1 * m);
    }
    f *= 1.0 - 0.5 * crack(vec2<f32>(tx, ty), vec2<f32>(1.0, 2.0), id, select(0.12, 0.2, dungeon), px);
    return f;
}

fn floor_top(p: vec2<f32>, c: vec2<i32>, v: f32, px: f32) -> vec3<f32> {
    let dungeon = wear.kind.y < 0.5;
    let id = vec2<f32>(c) + vec2<f32>(wear.kind.z, wear.kind.z * 0.37) + 0.5;
    let tx = fract(p.x);
    var f = tone(p, id, 0.13);
    // dirt gathers against the back wall and in the seams between tiles
    f *= 1.0 - 0.2 * (1.0 - smoothstep(0.0, 0.3, v)) - 0.05 * (1.0 - smoothstep(0.0, 0.06, min(tx, 1.0 - tx)));
    f *= 1.0 - 0.05 * smoothstep(0.88, 1.0, v);
    if dungeon {
        f *= 1.0 - 0.18 * smoothstep(0.55, 0.78, fbm(p * vec2<f32>(1.6, 5.0) + wear.kind.z));
        {
            let reach = 0.16 + 0.2 * fbm(vec2<f32>(p.x * 3.0, wear.kind.z));
            let m = (1.0 - smoothstep(0.0, reach, v + (vnoise(p * 16.0) - 0.5) * 0.14)) * smoothstep(0.35, 0.6, fbm(vec2<f32>(p.x * 0.8, wear.kind.z + 7.0)));
            f *= moss(m * 0.7) * (1.0 - 0.12 * m);
        }
    } else {
        // golden dust gathered at the back
        f *= mix(vec3<f32>(1.0), vec3<f32>(1.0, 0.94, 0.8), 0.14 * (1.0 - smoothstep(0.0, 0.5, v)));
        // the polished marble throws a torch's glow back as a soft warm streak across the floor in front of it (the flame is too
        // high above the floor to be seen in it)
        var sheen = 0.0;
        for (var i = 0; i < 4; i++) {
            let t = wear.torch[i];
            if t.z > 0.0 && i32(floor(t.y / 2.0)) == 2 - c.y {
                let dx = (p.x - t.x) / 0.42;
                sheen += exp(-dx * dx) * (0.35 + 0.65 * sin(3.1416 * v));
            }
        }
        f *= mix(vec3<f32>(1.0), vec3<f32>(1.5, 1.2, 0.8), 0.3 * clamp(sheen, 0.0, 1.0));
    }
    f *= 1.0 - 0.4 * crack(vec2<f32>(tx, v * 1.4), vec2<f32>(1.0, 1.4), id, select(0.08, 0.14, dungeon), px);
    return f;
}

fn back_wall(p: vec2<f32>, c: vec2<i32>, yl: f32, px: f32) -> vec3<f32> {
    let dungeon = wear.kind.y < 0.5;
    let id = vec2<f32>(c) + vec2<f32>(wear.kind.z, wear.kind.z * 0.37) + 0.25;
    let tx = fract(p.x);
    let up = yl - SLAB; // above the floor line
    let top = 2.0 - yl; // below the ceiling
    var f = tone(p, id, 0.13);
    // damp and moss rising from the floor line, grime under the ceiling
    let rise = (0.25 + 0.5 * fbm(vec2<f32>(p.x * 1.7, wear.kind.z))) * select(0.35, 1.0, dungeon);
    let wet = 1.0 - smoothstep(0.0, rise, up);
    f *= 1.0 - 0.22 * wet - 0.2 * (1.0 - smoothstep(0.0, 0.35, top));
    // moss in the crease where floor tiles meet the wall (only where there is a floor, fading out over the neighbouring column)
    // and in soft patches on the wall itself; every edge is a wide smoothstep, nothing ends in a line
    if dungeon {
        let fl = max(select(0.0, 1.0, floored(c)), max(select(0.0, 1.0, floored(c + vec2<i32>(-1, 0))) * (1.0 - smoothstep(0.0, 0.3, tx)), select(0.0, 1.0, floored(c + vec2<i32>(1, 0))) * smoothstep(0.7, 1.0, tx)));
        let crease = (1.0 - smoothstep(0.0, 0.1 + 0.14 * fbm(vec2<f32>(p.x * 3.0, wear.kind.z)), up + (vnoise(p * 9.0) - 0.5) * 0.08)) * fl * smoothstep(0.3, 0.6, fbm(vec2<f32>(p.x * 0.8, wear.kind.z + 3.0)));
        let blotch = smoothstep(0.55, 0.9, fbm(p * 1.4 + wear.kind.z * 1.7)) * (0.7 + 0.3 * vnoise(p * 9.0)) * 0.55;
        let m = max(crease, blotch);
        f *= moss(m * 0.75) * (1.0 - 0.15 * m);
    }
    // stains running down from the ceiling gaps
    f *= 1.0 - 0.18 * streak(tx, top * 0.5, id, select(0.2, 0.3, dungeon), 0.03);
    // soot above each torch flame, brown-gold in the palace
    for (var i = 0; i < 4; i++) {
        let t = wear.torch[i];
        if t.z <= 0.0 {
            continue;
        }
        let dx = (p.x - t.x) / 0.3;
        let dy = p.y - t.y;
        let soot = exp(-dx * dx) * smoothstep(0.0, 0.2, dy) * (1.0 - smoothstep(0.3, 1.5, dy)) * (0.7 + 0.3 * vnoise(p * 9.0));
        f *= mix(vec3<f32>(1.0), select(vec3<f32>(0.78, 0.7, 0.55), vec3<f32>(0.62, 0.6, 0.58), dungeon), soot * 0.7);
    }
    // cobwebs in the upper corner next to a wall block, here and there (dungeon only)
    if dungeon && hash21(id + 9.0) > 0.45 {
        var w = 0.0;
        if solid(c + vec2<i32>(-1, 0)) {
            w = web(vec2<f32>(p.x, yl), vec2<f32>(f32(c.x), 2.0), -1.0);
        } else if solid(c + vec2<i32>(1, 0)) {
            w = web(vec2<f32>(p.x, yl), vec2<f32>(f32(c.x) + 1.0, 2.0), 1.0);
        }
        f *= 1.0 + 1.1 * w;
    }
    f *= 1.0 - 0.45 * crack(vec2<f32>(tx, yl), vec2<f32>(1.0, 2.0), id + 5.0, select(0.05, 0.1, dungeon), px);
    return f;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xy - wear.org.xy;
    let px = max(length(fwidth(p)), 1e-4);
    let yy = p.y + SLAB;
    let k = floor(yy / 2.0);
    let c = vec2<i32>(i32(floor(p.x)), 2 - i32(k));
    if c.x < 0 || c.x > 9 || c.y < 0 || c.y > 2 {
        discard;
    }
    let yl = yy - 2.0 * k;
    var f = vec3<f32>(1.0);
    if wear.kind.x < 0.5 {
        if !solid(c) {
            discard;
        }
        f = wall_face(p, c, yl, px);
    } else if wear.kind.x < 1.5 {
        if !floored(c) {
            discard;
        }
        f = floor_top(p, c, clamp((in.world_position.z + 0.9) / 1.4, 0.0, 1.0), px);
    } else {
        if solid(c) {
            discard;
        }
        f = back_wall(p, c, yl, px);
    }
    return vec4<f32>(f, 1.0);
}
