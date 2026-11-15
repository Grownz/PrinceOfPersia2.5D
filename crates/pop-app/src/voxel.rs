//! Voxel characters (docs/render_roadmap.md V1): each original sprite image is extruded at runtime into a small voxel grid
//! (never written to disk) and meshed into cube faces with vertex colours, so lights shade it like the rooms.
//! Depth profile: "inflated" extrusion. A pixel's column is 2k + 1 voxels thick, centred on the sprite plane, where k is its
//! 4-neighbour distance to the silhouette edge capped at MAX_STEPS (edge 3, then 5, inner body 7). A constant thickness reads
//! as a cardboard cut-out from the tilted camera; the rounded profile gives torso and head some body while thin limbs stay
//! thin. Every voxel of a column has the pixel's palette index, so the front projection is exactly the sprite.
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Cap on the edge distance used for the thickness: depth = 2 * MAX_STEPS + 1 voxels.
pub const MAX_STEPS: usize = 3;

/// A voxel grid: `voxels[x + w * (y + h * z)]` = palette index (0 = empty). y = 0 is the image's top row, z = d - 1 the
/// front layer (towards the camera).
pub struct VoxelModel {
    pub w: usize,
    pub h: usize,
    pub d: usize,
    pub voxels: Vec<u8>,
}

impl VoxelModel {
    /// Extrude a w x h image of palette indices (row-major, 0 = transparent). `mirrored` flips it left-right (the engine
    /// mirrors a character facing right).
    pub fn from_image(w: usize, h: usize, pixels: &[u8], mirrored: bool) -> Self {
        let px = |x: usize, y: usize| pixels[y * w + if mirrored { w - 1 - x } else { x }];
        // edge distance by repeated 4-neighbour erosion (outside the image counts as empty)
        let mut dist: Vec<usize> = (0..w * h).map(|i| (px(i % w, i / w) != 0) as usize).collect();
        for k in 2..=MAX_STEPS {
            let prev = dist.clone();
            let at = |x: isize, y: isize| if x < 0 || y < 0 || x >= w as isize || y >= h as isize { 0 } else { prev[y as usize * w + x as usize] };
            for (i, d) in dist.iter_mut().enumerate() {
                let (x, y) = ((i % w) as isize, (i / w) as isize);
                if *d == k - 1 && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().all(|&(dx, dy)| at(x + dx, y + dy) >= k - 1) {
                    *d = k;
                }
            }
        }
        let d = 2 * MAX_STEPS + 1;
        let mut voxels = vec![0; w * h * d];
        for y in 0..h {
            for x in 0..w {
                let k = dist[y * w + x];
                for z in (MAX_STEPS - k)..=(MAX_STEPS + k) {
                    voxels[x + w * (y + h * z)] = px(x, y);
                }
            }
        }
        VoxelModel { w, h, d, voxels }
    }

    fn get(&self, x: isize, y: isize, z: isize) -> u8 {
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
    /// RGBA). Units: one voxel = 1; x 0..w, y 0..h with the image's bottom line at y = 0, z centred on 0.
    pub fn mesh(&self, colour: impl Fn(u8) -> [f32; 4]) -> Mesh {
        const DIRS: [[isize; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        let (mut pos, mut nrm, mut col, mut idx) = (vec![], vec![], vec![], vec![]);
        let zc = self.d as f32 / 2.0;
        for z in 0..self.d as isize {
            for y in 0..self.h as isize {
                for x in 0..self.w as isize {
                    let v = self.get(x, y, z);
                    if v == 0 {
                        continue;
                    }
                    let c = colour(v);
                    // voxel cube in mesh space: image row y (top = 0) -> height h - 1 - y .. h - y
                    let o = Vec3::new(x as f32, (self.h as isize - 1 - y) as f32, z as f32 - zc);
                    for dir in DIRS {
                        // grid y grows downwards, mesh y upwards
                        if self.get(x + dir[0], y - dir[1], z + dir[2]) != 0 {
                            continue;
                        }
                        let n = Vec3::new(dir[0] as f32, dir[1] as f32, dir[2] as f32);
                        // two axes spanning the face, ordered so the quad winds counter-clockwise seen from outside
                        let (mut u, mut w) = (Vec3::new(n.y.abs(), n.z.abs(), n.x.abs()), Vec3::new(n.z.abs(), n.x.abs(), n.y.abs()));
                        if u.cross(w).dot(n) < 0.0 {
                            std::mem::swap(&mut u, &mut w);
                        }
                        let base = o + (n.max(Vec3::ZERO)); // the face lies on the cube side the normal points to
                        let i0 = pos.len() as u32;
                        for p in [base, base + u, base + u + w, base + w] {
                            pos.push(p.to_array());
                            nrm.push(n.to_array());
                            col.push(c);
                        }
                        idx.extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
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

    #[test]
    fn inflated_profile_and_mirroring_on_a_small_image() {
        // 5 x 5 filled square with one hole: edge pixels 3 deep, the next ring 5, the centre 7 (the cap)
        let mut px = vec![1u8; 25];
        px[0] = 0;
        px[24] = 7;
        let m = VoxelModel::from_image(5, 5, &px, false);
        assert_eq!(m.front(), px);
        let depth = |x, y| (0..m.d as isize).filter(|&z| m.get(x, y, z) != 0).count();
        assert_eq!((depth(1, 0), depth(1, 1), depth(2, 2), depth(4, 4)), (3, 5, 7, 3));
        let mm = VoxelModel::from_image(5, 5, &px, true);
        assert_eq!(mm.front(), mirror(5, &px));
        assert_eq!(mm.get(4, 0, 3), 0, "hole moved to the right");
    }

    #[test]
    fn faces_wind_counter_clockwise_around_their_normals() {
        let m = VoxelModel::from_image(2, 3, &[1, 0, 1, 1, 0, 1], false);
        let mesh = m.mesh(|_| [1.0; 4]);
        let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        let Some(VertexAttributeValues::Float32x3(n)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
        for t in idx.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(p[t[i] as usize]));
            assert!((b - a).cross(c - a).dot(Vec3::from(n[t[0] as usize])) > 0.0);
        }
        // bottom line at y = 0, top at h, depth centred on 0
        let (ymin, ymax) = p.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v[1]), hi.max(v[1])));
        let zmax = p.iter().fold(f32::MIN, |hi, v| hi.max(v[2]));
        assert_eq!((ymin, ymax, zmax), (0.0, 3.0, 1.5));
    }

    /// Acceptance (V1): the front projection equals the original image (silhouette and palette indices), as stored and
    /// mirrored, for every kid image and every guard-type image that decodes. Skips without Original_data.
    #[test]
    fn front_projection_equals_every_original_sprite() {
        let Some(kid) = data("KID.DAT") else { return };
        let t = std::time::Instant::now();
        let mut n = 0;
        for id in 401..=620u16 {
            let Some(im) = kid.get(id).and_then(image::decode) else { continue };
            for mirrored in [false, true] {
                let m = VoxelModel::from_image(im.width, im.height, &im.pixels, mirrored);
                let want = if mirrored { mirror(im.width, &im.pixels) } else { im.pixels.clone() };
                assert_eq!(m.front(), want, "kid image {id} mirrored {mirrored}");
                let _ = m.mesh(|_| [1.0; 4]);
            }
            n += 1;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("voxelised and meshed {n} kid images (both facings) in {ms:.1} ms");
        assert!(n >= 200, "only {n} kid images decoded");
        let mut g = 0;
        for f in ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"] {
            let Some(dat) = data(f) else { continue };
            for id in 751..791u16 {
                let Some(im) = dat.get(id).and_then(image::decode) else { continue };
                let m = VoxelModel::from_image(im.width, im.height, &im.pixels, true);
                assert_eq!(m.front(), mirror(im.width, &im.pixels), "{f} image {id}");
                g += 1;
            }
        }
        // sword images (PRINCE.DAT 701+)
        if let Some(p) = data("PRINCE.DAT") {
            for id in 701..751u16 {
                if let Some(im) = p.get(id).and_then(image::decode) {
                    assert_eq!(VoxelModel::from_image(im.width, im.height, &im.pixels, false).front(), im.pixels, "sword {id}");
                }
            }
        }
        assert!(g > 100, "only {g} guard-type images decoded");
    }
}
