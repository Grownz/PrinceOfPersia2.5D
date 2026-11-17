//! Voxel characters (docs/render_roadmap.md V1/V2): each original sprite image is extruded at runtime into a small voxel
//! grid (never written to disk) and meshed into cube faces with vertex colours, so lights shade it like the rooms.
//! Depth profile (V2): a union of balls. Every sprite pixel is the centre of a ball whose radius is its distance to the
//! silhouette edge (capped, so the torso stays a rounded slab); a column's front height is the highest ball over it. Heads
//! come out sphere-like, limbs as cylinders as thick as they are wide, hands and feet thin. Colour-aware on top: each
//! colour region (skin, cloth, hair, sash, by hue class) adds its own small rounded bump so parts read separately, darker
//! shades of a region (cloth folds) sit a little deeper, and the back is a flatter, thinner copy of the front profile.
//! Every voxel of a column has the pixel's palette index, so the front projection is exactly the sprite.
//! Mesh: per-face culling, flat normals, vertex colours with baked ambient occlusion, rear layers darker and desaturated,
//! and a small per-voxel brightness jitter so the cubes read as cubes.
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Ball radius cap (sprite pixels): the torso's half thickness in front of the sprite plane.
const CAP: f32 = 4.0;
/// Colour-region bump: radius cap (sprite px) and weight.
const REGION_CAP: f32 = 1.5;
const REGION_BUMP: f32 = 0.6;
/// Darker shades of a colour region (below this fraction of its brightest luminance) are pushed back by RECESS px.
const SHADE: f32 = 0.8;
const RECESS: f32 = 0.6;
/// Back profile: this fraction of the silhouette profile, capped (flatter back).
const BACK: f32 = 0.6;
const BACK_CAP: f32 = 2.0;
/// Ambient occlusion per vertex by occluder count (0..3), and the jitter amplitude.
const AO: [f32; 4] = [1.0, 0.8, 0.66, 0.54];
const JITTER: f32 = 0.06;
const COLUMN_JITTER: f32 = 0.02;

/// First and last opaque pixel of a row that is one opaque run (None: empty row).
fn run_of(row: &[u8]) -> Option<(usize, usize)> {
    Some((row.iter().position(|&p| p != 0)?, row.iter().rposition(|&p| p != 0)?))
}

/// A voxel grid: `voxels[x + w * (y + h * z)]` = palette index (0 = empty). y = 0 is the image's top row, z = d - 1 the
/// front layer (towards the camera), z = `plane` the sprite plane.
pub struct VoxelModel {
    pub w: usize,
    pub h: usize,
    pub d: usize,
    pub plane: usize,
    pub voxels: Vec<u8>,
    /// per-voxel brightness jitter amplitude of the mesh (JITTER; columns use less, they are smooth stone)
    pub jitter: f32,
}

/// Chamfer distance (orthogonal 1, diagonal sqrt 2) from the cells with `init` 0; outside the grid counts as 0.
fn chamfer(mut d: Vec<f32>, w: usize, h: usize) -> Vec<f32> {
    let at = |d: &[f32], x: isize, y: isize| if x < 0 || y < 0 || x >= w as isize || y >= h as isize { 0.0 } else { d[y as usize * w + x as usize] };
    let s2 = std::f32::consts::SQRT_2;
    for pass in [[(-1, 0, 1.0), (0, -1, 1.0), (-1, -1, s2), (1, -1, s2)], [(1, 0, 1.0), (0, 1, 1.0), (1, 1, s2), (-1, 1, s2)]] {
        let fwd = pass[0].0 < 0;
        for i in 0..w * h {
            let i = if fwd { i } else { w * h - 1 - i };
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let m = pass.iter().fold(d[i], |m, &(dx, dy, c)| m.min(at(&d, x + dx, y + dy) + c));
            d[i] = m;
        }
    }
    d
}

/// Union of balls: each cell q with `filled(q)` is a ball of radius min(dist(q) - 0.5, cap) on the plane; the height at p
/// is the highest ball surface over it, counting only balls with `same(p, q)`.
fn balls(dist: &[f32], w: usize, h: usize, cap: f32, filled: impl Fn(usize) -> bool, same: impl Fn(usize, usize) -> bool) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    for q in (0..w * h).filter(|&q| filled(q)) {
        let r = (dist[q] - 0.5).min(cap);
        let ri = r.ceil() as isize;
        let (qx, qy) = ((q % w) as isize, (q / w) as isize);
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (x, y) = (qx + dx, qy + dy);
                let s = r * r - (dx * dx + dy * dy) as f32;
                if s < 0.0 || x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                    continue;
                }
                let p = y as usize * w + x as usize;
                if filled(p) && same(p, q) {
                    out[p] = out[p].max(s.sqrt());
                }
            }
        }
    }
    out
}

/// Colour class of an sRGB colour: 0 neutral (white/grey cloth), 1 dark, 2 skin, 3.. hue sectors of 60 degrees.
fn class(c: [u8; 3]) -> u8 {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
    if mx < 0.2 {
        return 1;
    }
    if sat < 0.2 {
        return 0;
    }
    let hue = if mx == r { 60.0 * ((g - b) / (mx - mn)).rem_euclid(6.0) } else if mx == g { 60.0 * ((b - r) / (mx - mn) + 2.0) } else { 60.0 * ((r - g) / (mx - mn) + 4.0) };
    if hue < 35.0 && r > g && g > b && sat < 0.75 {
        return 2;
    }
    3 + (hue / 60.0) as u8
}

pub(crate) fn luma(c: [u8; 3]) -> f32 {
    (0.3 * c[0] as f32 + 0.59 * c[1] as f32 + 0.11 * c[2] as f32) / 255.0
}

impl VoxelModel {
    /// Extrude a w x h image of palette indices (row-major, 0 = transparent). `mirrored` flips it left-right (the engine
    /// mirrors a character facing right); `rgb` gives a palette index's colour for the colour-aware part of the profile.
    pub fn from_image(w: usize, h: usize, pixels: &[u8], mirrored: bool, rgb: impl Fn(u8) -> [u8; 3]) -> Self {
        let px: Vec<u8> = (0..w * h).map(|i| pixels[i / w * w + if mirrored { w - 1 - i % w } else { i % w }]).collect();
        let (mut cls, mut lum) = ([0u8; 256], [0f32; 256]);
        for p in 1..=255u8 {
            (cls[p as usize], lum[p as usize]) = (class(rgb(p)), luma(rgb(p)));
        }
        let filled = |i: usize| px[i] != 0;
        let cl = |i: usize| cls[px[i] as usize];
        // distance to the silhouette edge, and to the edge of the pixel's colour region
        let dt = chamfer(px.iter().map(|&p| if p == 0 { 0.0 } else { f32::MAX }).collect(), w, h);
        let edge = |i: usize| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize || px[ny as usize * w + nx as usize] == 0 || cl(ny as usize * w + nx as usize) != cl(i)
            })
        };
        let dr = chamfer((0..w * h).map(|i| if !filled(i) { 0.0 } else if edge(i) { 1.0 } else { f32::MAX }).collect(), w, h);
        let hs = balls(&dt, w, h, CAP, filled, |_, _| true);
        let hr = balls(&dr, w, h, REGION_CAP, filled, |p, q| cl(p) == cl(q));
        let mut brightest = [0f32; 256];
        for &p in px.iter().filter(|&&p| p != 0) {
            let c = cls[p as usize] as usize;
            brightest[c] = brightest[c].max(lum[p as usize]);
        }
        // per column: voxels in front of / behind the plane voxel
        let cols: Vec<(usize, usize)> = (0..w * h)
            .map(|i| {
                if !filled(i) {
                    return (0, 0);
                }
                let shade = lum[px[i] as usize] < SHADE * brightest[cl(i) as usize];
                let hf = hs[i] + REGION_BUMP * hr[i] - if shade { RECESS } else { 0.0 };
                let hb = (BACK * hs[i]).min(BACK_CAP);
                ((hf - 0.5).round().max(0.0) as usize, (hb - 0.5).round().max(0.0) as usize)
            })
            .collect();
        let plane = cols.iter().map(|c| c.1).max().unwrap_or(0);
        let d = plane + 1 + cols.iter().map(|c| c.0).max().unwrap_or(0);
        let mut voxels = vec![0; w * h * d];
        for (i, &(nf, nb)) in cols.iter().enumerate() {
            if filled(i) {
                for z in plane - nb..=plane + nf {
                    voxels[i + w * h * z] = px[i];
                }
            }
        }
        VoxelModel { w, h, d, plane, voxels, jitter: JITTER }
    }

    /// A round column revolved about the vertical axis at image x `axis` (px, may be a half pixel): every row is a solid of
    /// revolution whose half-widths left and right of the axis are the row's extent in the image (an ellipse when the two
    /// differ), depth radius their mean. A voxel takes the pixel colour of its image column: on a cylinder seen from the
    /// front the point at angle a projects to x = r sin a, so the front shows the original shading band, the back the same
    /// band (darkened by the mesh). Rows must be one opaque run each; the front projection is exactly `pixels`.
    pub fn revolve(w: usize, h: usize, pixels: &[u8], axis: f32) -> Self {
        let runs: Vec<Option<(usize, usize)>> = (0..h).map(|y| run_of(&pixels[y * w..][..w])).collect();
        let rz = |r: (usize, usize)| ((r.1 + 1) as f32 - r.0 as f32) / 2.0;
        let d = 2 * runs.iter().flatten().map(|&r| rz(r).ceil() as usize).max().unwrap_or(0).max(1);
        let mut voxels = vec![0; w * h * d];
        for (y, r) in runs.iter().enumerate() {
            let Some((x0, x1)) = *r else { continue };
            let (left, right, rz) = (axis - x0 as f32, (x1 + 1) as f32 - axis, rz((x0, x1)));
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - axis;
                let rx = if dx < 0.0 { left } else { right };
                for z in 0..d {
                    let dz = z as f32 + 0.5 - d as f32 / 2.0;
                    // the two layers next to the axis plane are always kept, so the silhouette is the image's
                    if dz.abs() <= 0.5 || (dx / rx).powi(2) + (dz / rz).powi(2) <= 1.0 {
                        voxels[x + w * (y + h * z)] = pixels[y * w + x];
                    }
                }
            }
        }
        VoxelModel { w, h, d, plane: d / 2, voxels, jitter: COLUMN_JITTER }
    }

    /// A square block column (the dungeon pillar: front face and right side face side by side in the image, split at image
    /// column `split`): a block `split` px wide and as deep, the front face is the image left of the split, the right side
    /// face the image right of it stretched over the depth; inside and the other faces take the front colours.
    pub fn block(w: usize, h: usize, pixels: &[u8], split: usize) -> Self {
        let d = split;
        let mut voxels = vec![0; split * h * d];
        for y in 0..h {
            let row = &pixels[y * w..][..w];
            for x in (0..split).filter(|&x| row[x] != 0) {
                for z in 0..d {
                    // depth k from the front (0) to the back: side pixel s + k * (w - s) / d
                    let side = row[split + (d - 1 - z) * (w - split) / d];
                    voxels[x + split * (y + h * z)] = if x == split - 1 && z + 1 < d && side != 0 { side } else { row[x] };
                }
            }
        }
        VoxelModel { w: split, h, d, plane: d / 2, voxels, jitter: COLUMN_JITTER }
    }

    /// Thin round rods (spikes, flames, torch holder): every opaque run of every row is revolved about its own centre, a
    /// disc as deep as the run is wide; the two layers at the plane are always kept, so the front projection is `pixels`.
    pub fn revolve_runs(w: usize, h: usize, pixels: &[u8]) -> Self {
        let runs: Vec<(usize, usize, usize)> = (0..h)
            .flat_map(|y| {
                let row = &pixels[y * w..][..w];
                let mut v = vec![];
                let mut x = 0;
                while x < w {
                    if row[x] == 0 {
                        x += 1;
                        continue;
                    }
                    let a = x;
                    while x < w && row[x] != 0 {
                        x += 1;
                    }
                    v.push((y, a, x - 1));
                }
                v
            })
            .collect();
        let d = (2 * runs.iter().map(|&(_, a, b)| (b + 2 - a) / 2).max().unwrap_or(1)).max(2);
        let mut voxels = vec![0; w * h * d];
        for &(y, a, b) in &runs {
            let (c, r) = ((a + b + 1) as f32 / 2.0, (b + 1 - a) as f32 / 2.0);
            for x in a..=b {
                let dx = x as f32 + 0.5 - c;
                for z in 0..d {
                    let dz = z as f32 + 0.5 - d as f32 / 2.0;
                    if dz.abs() <= 0.5 || dx * dx + dz * dz <= r * r {
                        voxels[x + w * (y + h * z)] = pixels[y * w + x];
                    }
                }
            }
        }
        VoxelModel { w, h, d, plane: d / 2, voxels, jitter: COLUMN_JITTER }
    }

    /// Extrusion by per-pixel layer counts: `layers(palette index, distance to the silhouette edge)` = (layers in front of
    /// the plane, layers behind it). Every voxel of a column has the pixel's index, so the front projection is `pixels`.
    pub fn extrude(w: usize, h: usize, pixels: &[u8], jitter: f32, layers: impl Fn(u8, f32) -> (usize, usize)) -> Self {
        let dt = chamfer(pixels.iter().map(|&p| if p == 0 { 0.0 } else { f32::MAX }).collect(), w, h);
        let cols: Vec<(usize, usize)> = (0..w * h).map(|i| if pixels[i] == 0 { (0, 0) } else { layers(pixels[i], dt[i]) }).collect();
        let plane = cols.iter().map(|c| c.1).max().unwrap_or(0);
        let d = plane + 1 + cols.iter().map(|c| c.0).max().unwrap_or(0);
        let mut voxels = vec![0; w * h * d];
        for (i, &(nf, nb)) in cols.iter().enumerate().filter(|(i, _)| pixels[*i] != 0) {
            for z in plane - nb..=plane + nf {
                voxels[i + w * h * z] = pixels[i];
            }
        }
        VoxelModel { w, h, d, plane, voxels, jitter }
    }

    /// A bevelled plate (blades, door frames and panels): half thickness min(edge distance - 1, `cap`) voxels each side, so
    /// the rim is one voxel thick; `relief(palette index)` adds layers in front (brighter ornament stands out).
    pub fn plate(w: usize, h: usize, pixels: &[u8], cap: f32, relief: impl Fn(u8) -> usize) -> Self {
        Self::extrude(w, h, pixels, COLUMN_JITTER, |p, dt| {
            let n = (dt - 1.0).clamp(0.0, cap).round() as usize;
            (n + relief(p), n)
        })
    }

    /// The model with every image column x pushed forward by round((x_ref - x) * k) voxels: a plate standing diagonally
    /// through the depth (the 2D chomper blade is drawn sheared, its left edge in front of the figures). Front unchanged.
    pub fn sheared(&self, k: f32, x_ref: f32) -> Self {
        let shift = |x: usize| ((x_ref - x as f32) * k).round().max(0.0) as usize;
        let extra = (0..self.w).map(shift).max().unwrap_or(0);
        let d = self.d + extra;
        let mut voxels = vec![0; self.w * self.h * d];
        for z in 0..self.d {
            for y in 0..self.h {
                for x in 0..self.w {
                    voxels[x + self.w * (y + self.h * (z + shift(x)))] = self.voxels[x + self.w * (y + self.h * z)];
                }
            }
        }
        VoxelModel { w: self.w, h: self.h, d, plane: self.plane, voxels, jitter: self.jitter }
    }

    /// Mesh z of the column axis (the depth centre of the grid), in voxels.
    pub fn axis_z(&self) -> f32 {
        self.d as f32 / 2.0 - self.plane as f32 - 0.5
    }

    pub(crate) fn get(&self, x: isize, y: isize, z: isize) -> u8 {
        if x < 0 || y < 0 || z < 0 || x >= self.w as isize || y >= self.h as isize || z >= self.d as isize {
            return 0;
        }
        self.voxels[x as usize + self.w * (y as usize + self.h * z as usize)]
    }

    /// What an orthographic camera in front sees: the first filled voxel of each column from the front (row-major, w x h).
    #[cfg(test)]
    pub fn front(&self) -> Vec<u8> {
        (0..self.w * self.h)
            .map(|i| (0..self.d as isize).rev().map(|z| self.get((i % self.w) as isize, (i / self.w) as isize, z)).find(|&v| v != 0).unwrap_or(0))
            .collect()
    }

    /// Cube faces between a filled and an empty voxel, flat normals, vertex colours from `colour(palette index)` (linear
    /// RGBA) with ambient occlusion, rear-layer darkening and jitter. Units: one voxel = 1; x 0..w, y 0..h with the image's
    /// bottom line at y = 0, z = 0 the centre of the sprite-plane voxel.
    pub fn mesh(&self, colour: impl Fn(u8) -> [f32; 4]) -> Mesh {
        const DIRS: [[isize; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        let (mut pos, mut nrm, mut col, mut idx) = (vec![], vec![], vec![], vec![]);
        let h = self.h as isize;
        // mesh-space lookup (y up)
        let filled = |x: isize, y: isize, z: isize| self.get(x, h - 1 - y, z) != 0;
        let front: Vec<isize> = (0..self.w * self.h).map(|i| (0..self.d as isize).rev().find(|&z| self.voxels[i + self.w * self.h * z as usize] != 0).unwrap_or(0)).collect();
        for z in 0..self.d as isize {
            for gy in 0..h {
                for x in 0..self.w as isize {
                    let v = self.get(x, gy, z);
                    if v == 0 {
                        continue;
                    }
                    let y = h - 1 - gy;
                    // rear layers: darker and greyer with the distance from the column's front (sprite px)
                    let k = (front[x as usize + self.w * gy as usize] - z) as f32;
                    let [r, g, b, a] = colour(v);
                    let l = 0.3 * r + 0.59 * g + 0.11 * b;
                    let (dim, grey) = ((1.0 - 0.08 * k).max(0.55), (0.07 * k).min(0.4));
                    let jit = 1.0 + self.jitter * (((x * 73_856_093) ^ (y * 19_349_663) ^ (z * 83_492_791)) as u32 % 1000) as f32 / 500.0 - self.jitter;
                    let base = [r, g, b].map(|c| (c + (l - c) * grey) * dim * jit);
                    let o = [x, y, z];
                    for dir in DIRS {
                        if filled(x + dir[0], y + dir[1], z + dir[2]) {
                            continue;
                        }
                        let n = Vec3::new(dir[0] as f32, dir[1] as f32, dir[2] as f32);
                        // two axes spanning the face, ordered so the quad winds counter-clockwise seen from outside
                        let (mut u, mut w) = ([dir[1].abs(), dir[2].abs(), dir[0].abs()], [dir[2].abs(), dir[0].abs(), dir[1].abs()]);
                        let v3 = |a: [isize; 3]| Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
                        if v3(u).cross(v3(w)).dot(n) < 0.0 {
                            std::mem::swap(&mut u, &mut w);
                        }
                        let (uf, wf) = (v3(u), v3(w));
                        let corner = Vec3::new(x as f32, y as f32, z as f32 - self.plane as f32 - 0.5) + n.max(Vec3::ZERO);
                        let i0 = pos.len() as u32;
                        let mut ao = [0usize; 4];
                        for (c, (su, sw)) in [(-1, -1), (1, -1), (1, 1), (-1, 1)].into_iter().enumerate() {
                            let cell = |a: isize, b: isize| filled(o[0] + dir[0] + a * u[0] + b * w[0], o[1] + dir[1] + a * u[1] + b * w[1], o[2] + dir[2] + a * u[2] + b * w[2]);
                            let (s1, s2) = (cell(su, 0), cell(0, sw));
                            ao[c] = if s1 && s2 { 3 } else { s1 as usize + s2 as usize + cell(su, sw) as usize };
                            let p = corner + uf * (su > 0) as u8 as f32 + wf * (sw > 0) as u8 as f32;
                            pos.push(p.to_array());
                            nrm.push(n.to_array());
                            let f = AO[ao[c]];
                            col.push([base[0] * f, base[1] * f, base[2] * f, a]);
                        }
                        // split the quad along the diagonal that keeps the occlusion gradient symmetric
                        if ao[0] + ao[2] < ao[1] + ao[3] {
                            idx.extend_from_slice(&[i0 + 1, i0 + 2, i0 + 3, i0 + 1, i0 + 3, i0]);
                        } else {
                            idx.extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
                        }
                    }
                }
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
            .with_inserted_indices(Indices::U32(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use pop_core::{dat::Dat, image};

    fn data(f: &str) -> Option<Dat> {
        Dat::open(format!("{}/../../Original_data/{f}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    fn mirror(w: usize, p: &[u8]) -> Vec<u8> {
        p.chunks(w).flat_map(|r| r.iter().rev().copied()).collect()
    }

    fn grey(p: u8) -> [u8; 3] {
        [p.wrapping_mul(40); 3]
    }

    /// (front, back) voxels of column (x, y) around the plane
    fn extent(m: &VoxelModel, x: isize, y: isize) -> (usize, usize) {
        let zs: Vec<isize> = (0..m.d as isize).filter(|&z| m.get(x, y, z) != 0).collect();
        ((*zs.last().unwrap() - m.plane as isize) as usize, (m.plane as isize - zs[0]) as usize)
    }

    #[test]
    fn rounded_profile_flatter_back_and_mirroring_on_a_small_image() {
        // 9 x 9 disc-ish square with a corner hole and a 1-px arm sticking out to the right on row 4
        let (w, h) = (11, 9);
        let mut px = vec![0u8; w * h];
        for y in 0..9 {
            for x in 0..9 {
                px[y * w + x] = 6;
            }
        }
        px[0] = 0;
        px[4 * w + 9] = 3;
        px[4 * w + 10] = 3;
        let m = VoxelModel::from_image(w, h, &px, false, grey);
        assert_eq!(m.front(), px);
        let (centre, edge, arm) = (extent(&m, 4, 4), extent(&m, 4, 0), extent(&m, 10, 4));
        assert!(centre.0 > edge.0 && edge.0 <= 2, "rounded: centre {centre:?} edge {edge:?}");
        assert!(centre.1 < centre.0, "flatter back {centre:?}");
        assert_eq!(arm, (0, 0), "1-px arm one voxel thick");
        let mm = VoxelModel::from_image(w, h, &px, true, grey);
        assert_eq!(mm.front(), mirror(w, &px));
        assert_eq!(mm.get(w as isize - 1, 0, mm.plane as isize), 0, "hole moved to the right");
    }

    #[test]
    fn faces_wind_counter_clockwise_and_rear_faces_are_darker() {
        let mut px = vec![1u8; 81];
        (px[0], px[40]) = (0, 2);
        let m = VoxelModel::from_image(9, 9, &px, false, |_| [200, 200, 200]);
        let mesh = m.mesh(|p| if p == 2 { [1.0, 0.0, 0.0, 1.0] } else { [1.0; 4] }); // centre column red
        let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        let Some(VertexAttributeValues::Float32x3(n)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
        let Some(VertexAttributeValues::Float32x4(c)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else { panic!() };
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
        for t in idx.chunks(3) {
            let [a, b, cc] = [0, 1, 2].map(|i| Vec3::from(p[t[i] as usize]));
            assert!((b - a).cross(cc - a).dot(Vec3::from(n[t[0] as usize])) > 0.0);
        }
        let (ymin, ymax) = p.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v[1]), hi.max(v[1])));
        assert_eq!((ymin, ymax), (0.0, 9.0));
        // the centre column: its back face is several layers behind its front face
        let best = |nz: f32| (0..p.len()).filter(|&i| n[i][2] == nz && c[i][1] == 0.0).map(|i| c[i][0]).fold(0.0, f32::max);
        assert!(best(1.0) > best(-1.0) * 1.15, "front {} back {}", best(1.0), best(-1.0));
    }

    /// Acceptance (V1/V2): the front projection equals the original image (silhouette and palette indices), as stored and
    /// mirrored, for every kid image and every guard-type image that decodes, plus the swords and the cutscene actors.
    /// Skips without Original_data.
    #[test]
    fn front_projection_equals_every_original_sprite() {
        let Some(kid) = data("KID.DAT") else { return };
        let pal = image::palette(kid.get(400).unwrap()).unwrap();
        let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        let t = std::time::Instant::now();
        let (mut n, mut faces) = (0, 0);
        for id in 401..=620u16 {
            let Some(im) = kid.get(id).and_then(image::decode) else { continue };
            for mirrored in [false, true] {
                let m = VoxelModel::from_image(im.width, im.height, &im.pixels, mirrored, rgb);
                let want = if mirrored { mirror(im.width, &im.pixels) } else { im.pixels.clone() };
                assert_eq!(m.front(), want, "kid image {id} mirrored {mirrored}");
                faces += m.mesh(|_| [1.0; 4]).count_vertices() / 4;
            }
            n += 1;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("voxelised and meshed {n} kid images (both facings) in {ms:.1} ms, {} faces per image", faces / (2 * n));
        assert!(n >= 200, "only {n} kid images decoded");
        let mut g = 0;
        for f in ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"] {
            let Some(dat) = data(f) else { continue };
            for id in 751..791u16 {
                let Some(im) = dat.get(id).and_then(image::decode) else { continue };
                let m = VoxelModel::from_image(im.width, im.height, &im.pixels, true, rgb);
                assert_eq!(m.front(), mirror(im.width, &im.pixels), "{f} image {id}");
                g += 1;
            }
        }
        // sword images (PRINCE.DAT 701+) and the cutscene actors (PV.DAT princess / Jaffar)
        if let Some(p) = data("PRINCE.DAT") {
            for id in 701..751u16 {
                if let Some(im) = p.get(id).and_then(image::decode) {
                    assert_eq!(VoxelModel::from_image(im.width, im.height, &im.pixels, false, rgb).front(), im.pixels, "sword {id}");
                }
            }
        }
        if let Some(pv) = data("PV.DAT") {
            let mut c = 0;
            for id in 800..980u16 {
                if let Some(im) = pv.get(id).and_then(image::decode) {
                    assert_eq!(VoxelModel::from_image(im.width, im.height, &im.pixels, true, rgb).front(), mirror(im.width, &im.pixels), "PV {id}");
                    c += 1;
                }
            }
            assert!(c > 20, "only {c} PV images decoded");
        }
        assert!(g > 100, "only {g} guard-type images decoded");
    }
}
