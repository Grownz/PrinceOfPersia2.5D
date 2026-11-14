//! Kid viewer: plays animation sequences with the original sprites (KID.DAT) so the extracted
//! sequence/frame tables can be checked by eye. Keys: , and . = previous/next sequence, Space = restart, F = flip.
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::{dat::Dat, exe::{Exe, Frame, SEQUENCES}, image, level::TILE_H_PX};
use std::collections::HashMap;

#[derive(Resource)]
pub struct KidAssets {
    pub exe: Exe,
    pub frames: Vec<Frame>,
    dat: Dat,
    pal: Vec<[u8; 3]>,
    /// image number -> (material, width px, height px)
    cache: HashMap<u8, (Handle<StandardMaterial>, f32, f32)>,
}

#[derive(Component)]
pub struct KidSprite;

impl KidAssets {
    pub fn load(dir: &str) -> Self {
        let exe = Exe::open(format!("{dir}/PRINCE.EXE")).expect("cannot open PRINCE.EXE (set POP_DATA)");
        let dat = Dat::open(format!("{dir}/KID.DAT")).expect("cannot open KID.DAT");
        let pal = image::palette(dat.get(400).unwrap()).unwrap();
        KidAssets { frames: exe.kid_frames(), exe, dat, pal, cache: HashMap::new() }
    }
}

pub fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, a: Res<KidAssets>, mut g: ResMut<Game>) {
    let seq = std::env::var("POP_SEQ").ok().and_then(|v| v.parse().ok()).filter(|s| (1..=SEQUENCES).contains(s)).unwrap_or(2);
    g.kid.start(&a.exe, seq);
    cmds.spawn((Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, KidSprite));
}

fn material_for(a: &mut KidAssets, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, n: u8) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    if let Some(c) = a.cache.get(&n) {
        return Some(c.clone());
    }
    let im = image::decode(a.dat.get(401 + n as u16)?)?;
    let mut rgba = Vec::with_capacity(im.pixels.len() * 4);
    for &p in &im.pixels {
        let c = a.pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        rgba.extend_from_slice(&[c[0], c[1], c[2], if p == 0 { 0 } else { 255 }]);
    }
    let mut img = Image::new(Extent3d { width: im.width as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: true, ..default() });
    let entry = (mat, im.width as f32, im.height as f32);
    a.cache.insert(n, entry.clone());
    Some(entry)
}

pub fn keys(k: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, a: Option<Res<KidAssets>>) {
    let Some(a) = a else { return };
    let step = k.just_pressed(KeyCode::Period) as i32 - k.just_pressed(KeyCode::Comma) as i32;
    if step != 0 {
        g.kid.seq = ((g.kid.seq as i32 - 1 + step).rem_euclid(SEQUENCES as i32) + 1) as usize;
    }
    if k.just_pressed(KeyCode::KeyF) {
        g.kid.facing_left = !g.kid.facing_left;
    }
    if step != 0 || k.just_pressed(KeyCode::Space) {
        let seq = g.kid.seq.max(1);
        g.kid.x = 0;
        g.kid.y = 0;
        g.kid_prev = (0, 0);
        g.kid.start(&a.exe, seq);
    }
}

/// Logic tick for the kid (called from the fixed-step system when a logic tick ran).
pub fn tick(g: &mut Game, a: &KidAssets) {
    g.kid_prev = (g.kid.x, g.kid.y);
    g.kid.tick(&a.exe, &a.frames);
    // viewer only: keep running animations on screen by wrapping around (remove when real movement exists)
    if g.kid.x.abs() > 63 {
        let d = -g.kid.x.signum() * 126;
        g.kid.x += d;
        g.kid_prev.0 += d;
    }
}

pub fn render(
    g: Res<Game>,
    a: Option<ResMut<KidAssets>>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<KidSprite>>,
) {
    let (Some(mut a), Ok((mut t, mut vis, mut mat))) = (a, q.single_mut()) else { return };
    let img_no = a.frames.get(g.kid.frame as usize).map_or(0, |f| f.image);
    let Some((m, w, h)) = (img_no != 255).then(|| material_for(&mut a, &mut imgs, &mut mats, img_no)).flatten() else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    mat.0 = m;
    // provisional placement: 14 logic units per tile, bottom row floor at y = 0; sprite anchored at its
    // left (facing right) or right (facing left) edge. Replace once the original placement rule is verified.
    let a_ = g.world.alpha();
    let lerp = |p: i32, c: i32| p as f32 + (c - p) as f32 * a_;
    let x = lerp(g.kid_prev.0, g.kid.x) / 14.0 + 4.5;
    let y = -lerp(g.kid_prev.1, g.kid.y) * 2.0 / TILE_H_PX as f32;
    let (sw, sh) = (w / 32.0, h * 2.0 / TILE_H_PX as f32);
    let dir = if g.kid.facing_left { -1.0 } else { 1.0 };
    t.translation = Vec3::new(x + dir * sw / 2.0, y + sh / 2.0, 0.6);
    t.scale = Vec3::new(dir * sw, sh, 1.0);
}
