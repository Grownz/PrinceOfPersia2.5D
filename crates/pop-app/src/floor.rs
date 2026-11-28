//! Polished palace marble (rides on the setting `atmosphere`, enhanced mode, palace levels): the floor materials get a low roughness
//! and some reflectance so the environment-map light on the camera (mood.rs) and the torch lights make real highlights. The
//! back wall's mirror image of the room and the figures is mirror.rs. The torches' glow on the floor is a shader sheen in wear.wesl.
use crate::room::Palette;
use crate::settings::{RenderMode, Settings};
use bevy::prelude::*;

/// Polished marble: perceptual roughness and reflectance of the palace floor materials (the classic look is 0.9 and 0.5).
pub const POLISH: (f32, f32) = (0.32, 0.65);
const MATTE: (f32, f32) = (0.9, 0.5);

/// Enhanced + atmosphere: the polish is on.
fn wanted(mode: &RenderMode, set: &Settings) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere
}

/// Every frame: the palace floors' roughness (live; also after the tile palette was rebuilt).
pub fn sync(mode: Res<RenderMode>, set: Res<Settings>, palette: Res<Palette>, mut std_mats: ResMut<Assets<StandardMaterial>>, mut on: Local<Option<bool>>) {
    let want = wanted(&mode, &set);
    // (never touched while the polish was never on: classic stays as it was)
    if on.unwrap_or(false) != want || (palette.is_changed() && want) {
        *on = Some(want);
        let (r, f) = if want { POLISH } else { MATTE };
        for h in palette.palace_floors() {
            if let Some(mut m) = std_mats.get_mut(&h) {
                (m.perceptual_roughness, m.reflectance) = (r, f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_is_smoother_than_matte() {
        assert!(POLISH.0 < MATTE.0 && POLISH.1 > MATTE.1);
    }
}
