//! The Controls page as pictures. Keyboard tab: a two-column list, the action on the left and its key caps (rounded
//! outlines with the key's legend) on the right, following the bindings of settings.json. Controller tab: one pad seen from
//! above with callout boxes and leader lines (Xbox or PlayStation, whichever is connected; Xbox when none is), built
//! from UI nodes in the menu's colours; nothing is loaded from disk.
use crate::menu::{Page, Tab};
use crate::menu_ui::{font, MenuUi};
use crate::settings::{Bindings, PadLayout, Settings};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Move,
    Action,
    Game,
    View,
}

fn colour(g: Group) -> Color {
    match g {
        Group::Move => Color::srgb(1.0, 0.85, 0.5),
        Group::Action => Color::srgb(0.96, 0.46, 0.36),
        Group::Game => Color::srgb(0.48, 0.72, 1.0),
        Group::View => Color::srgb(0.52, 0.86, 0.56),
    }
}

const CAP_EDGE: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);
// unused parts are opaque and drawn over the leader lines, so a line passes behind them
const DIM_BG: Color = Color::srgb(0.2, 0.2, 0.25);
const DIM_EDGE: Color = Color::srgb(0.38, 0.38, 0.44);
const DIM_TEXT: Color = Color::srgb(0.8, 0.8, 0.85);
const INK: Color = Color::srgb(0.04, 0.04, 0.07);
const CARD_BG: Color = Color::srgb(0.02, 0.02, 0.04);

/// The picture of one tab (shown while that tab of the Controls page is open).
#[derive(Component)]
pub struct ArtRoot(Tab);

/// Which pad drawing this is (`true` = PlayStation).
#[derive(Component)]
pub struct PadRoot(bool, PadLayout);

/// Text under the pad drawing (which pad is shown).
#[derive(Component)]
pub struct PadNote;

/// The action label of a keyboard row (column, row).
#[derive(Component)]
pub struct RowLabel(usize, usize);

/// Key cap slot k of a keyboard row.
#[derive(Component)]
pub struct Slot(usize, usize, usize);

const SLOTS: usize = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PadKind {
    Xbox,
    PlayStation,
}

/// Which family a connected pad belongs to, by USB vendor (Sony 054C, Microsoft 045E) or else by its name; `None` = unknown.
pub fn pad_kind(vendor: Option<u16>, name: &str) -> Option<PadKind> {
    let n = name.to_lowercase();
    match vendor {
        Some(0x054C) => return Some(PadKind::PlayStation),
        Some(0x045E) => return Some(PadKind::Xbox),
        _ => {}
    }
    if ["playstation", "dualshock", "dualsense", "ps3", "ps4", "ps5", "sony", "wireless controller"].iter().any(|k| n.contains(k)) {
        Some(PadKind::PlayStation)
    } else if ["xbox", "x-box", "xinput", "microsoft"].iter().any(|k| n.contains(k)) {
        Some(PadKind::Xbox)
    } else {
        None
    }
}

/// A key as people write it: "ArrowLeft" -> "←", "ShiftLeft" -> "Shift", "KeyA" -> "A", "Numpad7" -> "Num 7".
pub fn pretty(code: &str) -> String {
    match code {
        "ArrowLeft" => "←".into(),
        "ArrowRight" => "→".into(),
        "ArrowUp" => "↑".into(),
        "ArrowDown" => "↓".into(),
        "ShiftLeft" | "ShiftRight" => "Shift".into(),
        "ControlLeft" | "ControlRight" => "Ctrl".into(),
        "AltLeft" | "AltRight" => "Alt".into(),
        c if c.starts_with("Key") && c.len() == 4 => c[3..].into(),
        c if c.starts_with("Digit") => c[5..].into(),
        c if c.starts_with("Numpad") => format!("Num {}", &c[6..]),
        c => c.into(),
    }
}

/// Caps of a row: the bound keys (distinct names), then the fixed extras, separated by "/".
fn caps(bound: &[String], extra: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for n in bound.iter().map(|n| pretty(n)).chain(extra.iter().map(|s| s.to_string())) {
        if !out.contains(&n) {
            if !out.is_empty() {
                out.push("/".into());
            }
            out.push(n);
        }
    }
    out
}

fn combo(keys: &[&str]) -> Vec<String> {
    keys.iter().flat_map(|k| ["+".to_string(), k.to_string()]).skip(1).collect()
}

/// The two columns of the keyboard tab: (action, key caps).
fn rows(b: &Bindings) -> [Vec<(&'static str, Vec<String>)>; 2] {
    [
        vec![
            ("Move left", caps(&b.left, &["Num 4"])),
            ("Move right", caps(&b.right, &["Num 6"])),
            ("Jump", caps(&b.up, &["Num 8"])),
            ("Crouch", caps(&b.down, &["Num 5", "Num 2"])),
            ("Running jump left", caps(&[], &["Num 7"])),
            ("Running jump right", caps(&[], &["Num 9"])),
            ("Grab, strike, drink", caps(&b.shift, &[])),
            ("Classic / enhanced", caps(&[], &["F5"])),
            ("Sprite / voxel", caps(&[], &["F6"])),
        ],
        vec![
            ("Pause menu", caps(&[], &["Esc", "P"])),
            ("Restart level", combo(&["Ctrl", "A"])),
            ("Save (levels 3-13)", combo(&["Ctrl", "G"])),
            ("Load (title)", combo(&["Ctrl", "L"])),
            ("Mute", caps(&[], &["M"])),
            ("Volume down / up", caps(&[], &["-", "="])),
            ("Fullscreen", caps(&[], &["F11"])),
            ("Frame times", caps(&[], &["F9"])),
        ],
    ]
}

fn vm(v: f32) -> Val {
    Val::VMin(v)
}

/// An absolutely placed box (vmin) with an optional centred legend.
#[allow(clippy::too_many_arguments)]
fn shape(p: &mut ChildSpawnerCommands, (x, y, w, h): (f32, f32, f32, f32), radius: f32, bg: Color, edge: Color, label: &str, ink: Color, size: f32) {
    let mut e = p.spawn((
        Node { position_type: PositionType::Absolute, left: vm(x), top: vm(y), width: vm(w), height: vm(h), border: UiRect::all(Val::Px(1.5)), border_radius: BorderRadius::all(vm(radius)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(bg),
        BorderColor::all(edge),
    ));
    if !label.is_empty() {
        e.with_children(|p| {
            p.spawn((Text::new(label), font(size), TextColor(ink)));
        });
    }
}

// ---- the pad ----

/// The drawing of each pad (white lines on transparent, made from the project's schematic) is embedded in the exe.
const XBOX_PNG: &[u8] = include_bytes!("../assets/pad_xbox.png");
const PS_PNG: &[u8] = include_bytes!("../assets/pad_ps.png");

/// The picture node of a pad drawing (`true` = PlayStation); `load` gives it its image.
#[derive(Component)]
pub struct PadImage(bool);

/// Where a marked part sits, in pixels of the pad drawing (about 600 x 400).
#[derive(Clone, Copy)]
enum Shape {
    Circle(f32, f32, f32),
    Rect(f32, f32, f32, f32),
}

impl Shape {
    fn centre(self) -> (f32, f32) {
        match self {
            Shape::Circle(x, y, _) => (x, y),
            Shape::Rect(x, y, w, h) => (x + w / 2.0, y + h / 2.0),
        }
    }
}

impl Shape {
    /// The shape placed in the picture, in vmin.
    fn in_vmin(self) -> Shape {
        match self {
            Shape::Circle(x, y, r) => {
                let (x, y) = px((x, y));
                Shape::Circle(x, y, r * K)
            }
            Shape::Rect(x, y, w, h) => {
                let (x, y) = px((x, y));
                Shape::Rect(x, y, w * K, h * K)
            }
        }
    }
}

/// Where the leader line of a callout at height `mid` ends on the outline of `shape` (picture vmin): a horizontal run to
/// the returned x at height `mid`, then vertical to the returned y. At the height of the part it meets the side facing the
/// callout, otherwise it goes to the middle of the top or bottom edge.
fn leader(shape: Shape, from_left: bool, mid: f32) -> (f32, f32) {
    match shape {
        Shape::Circle(cx, cy, r) => {
            let dy = mid - cy;
            if dy.abs() < r {
                let dx = (r * r - dy * dy).sqrt();
                (if from_left { cx - dx } else { cx + dx }, mid)
            } else {
                (cx, if dy < 0.0 { cy - r } else { cy + r })
            }
        }
        Shape::Rect(x, y, w, h) => {
            if mid >= y && mid <= y + h {
                (if from_left { x } else { x + w }, mid)
            } else {
                (x + w / 2.0, if mid < y { y } else { y + h })
            }
        }
    }
}

/// A part the game uses: outlined on the drawing in its group's colour and named by a callout.
struct Mark {
    text: String,
    left: bool,
    shape: Shape,
    g: Group,
}

const K: f32 = 0.097; // vmin per drawing pixel
const PAD_X: f32 = 23.5; // drawing left in the picture
const PAD_Y: f32 = 2.0;
const PIC_W: f32 = 105.0;
const PIC_H: f32 = 46.0;
const CALL_W: f32 = 23.0;
const CALL_H: f32 = 8.2;

/// The callouts of one pad drawing in one layout (settings pad_layout; the buttons' shapes are those of the drawings).
fn marks(ps: bool, layout: PadLayout) -> Vec<Mark> {
    use Shape::*;
    let n = |x: &str, y: &str| if ps { y.to_string() } else { x.to_string() };
    let m = |text: String, left, shape, g| Mark { text, left, shape, g };
    // (LB, left stick, D-pad, View, RB, Menu, X, B, A, Y) on the Xbox drawing, then on the PlayStation one
    let (lb, ls, dp, vw, rb, mn, bx, bb, ba, by) = if ps {
        (Rect(70.0, 6.0, 112.0, 26.0), Circle(212.0, 196.0, 66.0), Circle(115.0, 113.0, 52.0), Rect(152.0, 36.0, 24.0, 34.0), Rect(424.0, 6.0, 112.0, 26.0), Rect(434.0, 36.0, 24.0, 34.0), Circle(445.0, 115.0, 23.0), Circle(531.0, 115.0, 23.0), Circle(487.0, 157.0, 23.0), Circle(487.0, 73.0, 23.0))
    } else {
        (Rect(92.0, 20.0, 112.0, 30.0), Circle(152.0, 117.0, 66.0), Circle(220.0, 210.0, 50.0), Circle(257.0, 119.0, 16.0), Rect(392.0, 20.0, 112.0, 30.0), Circle(343.0, 119.0, 16.0), Circle(412.0, 121.0, 23.0), Circle(493.0, 121.0, 23.0), Circle(452.0, 158.0, 23.0), Circle(452.0, 84.0, 23.0))
    };
    let (l1, r1, x, b, a, y) = (n("LB", "L1"), n("RB", "R1"), n("X", "□"), n("B", "○"), n("A", "×"), n("Y", "△"));
    let (l2, r2) = (n("LT", "L2"), n("RT", "R2"));
    let view = m(format!("{}: restart level", n("View", "Create / Share")), true, vw, Group::Game);
    let menu = m(format!("{}: pause menu", n("Menu", "Options")), false, mn, Group::Game);
    match layout {
        PadLayout::Classic => vec![
            m(format!("{l1}: previous menu tab"), true, lb, Group::View),
            m("Left stick: move, jump, crouch".into(), true, ls, Group::Move),
            m("D-pad: move, jump, crouch".into(), true, dp, Group::Move),
            view,
            m(format!("{r1}: next menu tab"), false, rb, Group::View),
            menu,
            m(format!("{x}: action"), false, bx, Group::Action),
            m(format!("{b}: back in menus"), false, bb, Group::Game),
            m(format!("{a}: action, confirm"), false, ba, Group::Action),
        ],
        PadLayout::Combat => vec![
            m(format!("{l1} / {l2}: parry; menu: prev. tab"), true, lb, Group::Action),
            m("Left stick: move, jump, crouch".into(), true, ls, Group::Move),
            m("D-pad: move, jump, crouch".into(), true, dp, Group::Move),
            view,
            m(format!("{r1} / {r2}: action; menu: next tab"), false, rb, Group::Action),
            menu,
            m(format!("{x}: action, strike"), false, bx, Group::Action),
            m(format!("{b}: crouch; menus: back"), false, bb, Group::Move),
            m(format!("{a}: jump; menus: confirm"), false, ba, Group::Move),
        ],
        PadLayout::Modern => vec![
            m(format!("{l1}: previous menu tab"), true, lb, Group::View),
            m("Left stick: move, jump (no down)".into(), true, ls, Group::Move),
            m("D-pad: move, jump (no down)".into(), true, dp, Group::Move),
            view,
            m(format!("{r1} / {r2}: action; menu: next tab"), false, rb, Group::Action),
            menu,
            m(format!("{y}: parry"), false, by, Group::Action),
            m(format!("{x}: action, strike"), false, bx, Group::Action),
            m(format!("{b}: crouch; menus: back"), false, bb, Group::Move),
            m(format!("{a}: jump; menus: confirm"), false, ba, Group::Move),
        ],
    }
}

fn px(at: (f32, f32)) -> (f32, f32) {
    (PAD_X + at.0 * K, PAD_Y + at.1 * K)
}

/// The picture of one pad: the drawing, the outlines of the parts the game uses, callouts with leader lines.
fn pad(p: &mut ChildSpawnerCommands, ps: bool, layout: PadLayout) {
    let (iw, ih) = if ps { (606.0, 402.0) } else { (598.0, 405.0) };
    p.spawn((Node { display: Display::None, position_type: PositionType::Relative, width: vm(PIC_W), height: vm(PIC_H), ..default() }, PadRoot(ps, layout))).with_children(|p| {
        // a dark plate behind the line drawing, so the busy background does not cut through it
        p.spawn((Node { position_type: PositionType::Absolute, left: vm(PAD_X - 1.5), top: vm(PAD_Y - 0.5), width: vm(iw * K + 3.0), height: vm(ih * K + 1.0), border_radius: BorderRadius::all(vm(3.0)), ..default() }, BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.9))));
        p.spawn((ImageNode::default(), Node { position_type: PositionType::Absolute, left: vm(PAD_X), top: vm(PAD_Y), width: vm(iw * K), height: vm(ih * K), ..default() }, PadImage(ps)));
        let mut all = marks(ps, layout);
        for mk in &all {
            // the outline in the action's colour, a little outside the part
            let (x, y, w, h, r) = match mk.shape {
                Shape::Circle(cx, cy, r) => (cx - r, cy - r, 2.0 * r, 2.0 * r, r * K),
                Shape::Rect(x, y, w, h) => (x, y, w, h, 2.0),
            };
            let (x, y) = px((x, y));
            p.spawn((
                Node { position_type: PositionType::Absolute, left: vm(x), top: vm(y), width: vm(w * K), height: vm(h * K), border: UiRect::all(Val::Px(3.0)), border_radius: BorderRadius::all(vm(r)), ..default() },
                BorderColor::all(colour(mk.g)),
            ));
        }
        // callouts, ordered by the height of their part so the leader lines do not cross; the two sides are offset
        all.sort_by(|a, b| a.shape.centre().1.total_cmp(&b.shape.centre().1));
        for left in [true, false] {
            for (i, c) in all.iter().filter(|c| c.left == left).enumerate() {
                let step = if all.iter().filter(|c| c.left == left).count() > 5 { 7.4 } else if left { 9.4 } else { 8.6 };
                let cy = if left { 2.0 + i as f32 * step } else { i as f32 * step };
                let cx = if left { 0.0 } else { PIC_W - CALL_W };
                let col = colour(c.g);
                let edge_x = if left { CALL_W } else { PIC_W - CALL_W };
                let line = |x: f32, y: f32, w: f32, h: f32| (Node { position_type: PositionType::Absolute, left: vm(x), top: vm(y), width: vm(w), height: vm(h), ..default() }, BackgroundColor(col));
                let mid = cy + CALL_H / 2.0;
                // the line ends on the outline of the part or group, not in its middle
                let (tx, ty) = leader(c.shape.in_vmin(), left, mid);
                p.spawn(line(edge_x.min(tx), mid - 0.13, (tx - edge_x).abs(), 0.26)); // horizontal run
                p.spawn(line(tx - 0.13, mid.min(ty), 0.26, (ty - mid).abs())); // then up or down to the outline
                p.spawn((Node { position_type: PositionType::Absolute, left: vm(tx - 0.6), top: vm(ty - 0.6), width: vm(1.2), height: vm(1.2), border_radius: BorderRadius::all(vm(0.6)), ..default() }, BackgroundColor(col)));
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: vm(cx), top: vm(cy), width: vm(CALL_W), height: vm(CALL_H), padding: UiRect::axes(vm(0.8), vm(0.4)), border: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(vm(1.6)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(CARD_BG),
                    BorderColor::all(col),
                ))
                .with_children(|p| {
                    p.spawn((Text::new(c.text.clone()), font(1.8), TextColor(Color::WHITE), TextLayout::justify(Justify::Center)));
                });
            }
        }
    });
}

/// Gives the pad pictures their images (decoded from the embedded PNGs once the menu exists).
pub fn load(mut done: Local<bool>, mut images: ResMut<Assets<Image>>, mut q: Query<(&PadImage, &mut ImageNode)>) {
    if *done || q.is_empty() {
        return;
    }
    for (p, mut n) in &mut q {
        let bytes = if p.0 { PS_PNG } else { XBOX_PNG };
        match Image::from_buffer(bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::linear(), RenderAssetUsages::RENDER_WORLD) {
            Ok(img) => n.image = images.add(img),
            Err(e) => warn!("controller drawing: {e}"),
        }
    }
    *done = true;
}

/// Builds both tabs inside the menu card (hidden until `draw` shows the open tab's).
pub fn spawn(p: &mut ChildSpawnerCommands) {
    let root = |tab| (Node { display: Display::None, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vm(1.0), margin: UiRect::top(vm(1.2)), ..default() }, ArtRoot(tab));
    p.spawn(root(Tab::Keyboard)).with_children(|p| {
        p.spawn(Node { column_gap: vm(7.0), ..default() }).with_children(|p| {
            for col in 0..2 {
                p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vm(0.5), width: vm(46.0), ..default() }).with_children(|p| {
                    for row in 0..if col == 0 { 9 } else { 8 } {
                        p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, height: vm(4.9), ..default() }).with_children(|p| {
                            p.spawn((Text::new(""), font(2.3), TextColor(Color::WHITE), RowLabel(col, row)));
                            p.spawn(Node { column_gap: vm(0.7), align_items: AlignItems::Center, ..default() }).with_children(|p| {
                                for k in 0..SLOTS {
                                    p.spawn((Node { display: Display::None, padding: UiRect::axes(vm(1.0), vm(0.35)), border: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(vm(0.9)), ..default() }, BorderColor::all(CAP_EDGE), Slot(col, row, k))).with_children(|p| {
                                        p.spawn((Text::new(""), font(2.1), TextColor(Color::WHITE)));
                                    });
                                }
                            });
                        });
                    }
                });
            }
        });
    });
    p.spawn(root(Tab::Controller)).with_children(|p| {
        for l in [PadLayout::Modern, PadLayout::Combat, PadLayout::Classic] {
            pad(p, false, l);
            pad(p, true, l);
        }
        p.spawn((Text::new(""), font(1.9), TextColor(DIM_TEXT), PadNote));
    });
}

/// The pad to draw: the one connected (the first), else Xbox; POP_PAD=xbox|ps forces it. The name is `None` when none is connected.
fn current_pad(pads: &Query<(&Gamepad, Option<&Name>)>) -> (PadKind, Option<String>) {
    if let Ok(v) = std::env::var("POP_PAD") {
        return (if v == "ps" { PadKind::PlayStation } else { PadKind::Xbox }, Some(format!("forced by POP_PAD={v}")));
    }
    match pads.iter().next() {
        Some((g, name)) => {
            let name = name.map_or("controller", |n| n.as_str());
            (pad_kind(g.vendor_id(), name).unwrap_or(PadKind::Xbox), Some(name.to_string()))
        }
        None => (PadKind::Xbox, None),
    }
}

/// Every frame: show the open tab's picture, fill the keyboard rows from the bindings, pick the pad drawing.
#[allow(clippy::type_complexity)]
pub fn draw(
    ui: Res<MenuUi>,
    set: Res<Settings>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut roots: Query<(&ArtRoot, &mut Node), (Without<PadRoot>, Without<Slot>)>,
    mut pad_roots: Query<(&PadRoot, &mut Node), (Without<ArtRoot>, Without<Slot>)>,
    mut labels: Query<(&RowLabel, &mut Text), Without<PadNote>>,
    mut slots: Query<(&Slot, &Children, &mut Node, &mut BorderColor), (Without<ArtRoot>, Without<PadRoot>)>,
    mut texts: Query<&mut Text, (Without<RowLabel>, Without<PadNote>)>,
    mut note: Query<&mut Text, (With<PadNote>, Without<RowLabel>)>,
) {
    let tab = ui.menu.as_ref().filter(|m| m.page == Page::Controls).map(|m| m.tab);
    for (r, mut n) in &mut roots {
        let d = if tab == Some(r.0) { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    match tab {
        Some(Tab::Controller) => {
            let (kind, name) = current_pad(&pads);
            let layout = PadLayout::parse(&set.pad_layout);
            for (r, mut n) in &mut pad_roots {
                let d = if r.0 == (kind == PadKind::PlayStation) && r.1 == layout { Display::Flex } else { Display::None };
                if n.display != d {
                    n.display = d;
                }
            }
            let s = match name {
                Some(n) => format!("{n} ({})", if kind == PadKind::PlayStation { "PlayStation layout" } else { "Xbox layout" }),
                None => "No controller connected: showing the Xbox layout".into(),
            };
            for mut t in &mut note {
                if t.0 != s {
                    t.0 = s.clone();
                }
            }
        }
        Some(Tab::Keyboard) => {
            let table = rows(&set.keys);
            for (RowLabel(c, i), mut t) in &mut labels {
                let s = table[*c].get(*i).map_or("", |r| r.0);
                if t.0 != s {
                    t.0 = s.into();
                }
            }
            for (Slot(c, i, k), kids, mut n, mut edge) in &mut slots {
                let s = table[*c].get(*i).and_then(|r| r.1.get(*k)).map(String::as_str);
                let d = if s.is_some() { Display::Flex } else { Display::None };
                if n.display != d {
                    n.display = d;
                }
                let sep = matches!(s, Some("+" | "/"));
                edge.set_if_neq(BorderColor::all(if sep { Color::NONE } else { CAP_EDGE }));
                let pad = if sep { UiRect::axes(vm(0.2), vm(0.35)) } else { UiRect::axes(vm(1.0), vm(0.35)) };
                if n.padding != pad {
                    n.padding = pad;
                }
                for &t in kids {
                    if let (Ok(mut text), Some(s)) = (texts.get_mut(t), s) {
                        if text.0 != s {
                            text.0 = s.into();
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_the_bindings_and_names_are_readable() {
        let mut b = Bindings::default();
        let r = rows(&b);
        assert_eq!(r[0][0], ("Move left", vec!["←".to_string(), "/".into(), "Num 4".into()]));
        assert_eq!(r[0][6].1, vec!["Shift".to_string()], "left and right Shift are one cap");
        assert_eq!(r[1][1].1, vec!["Ctrl".to_string(), "+".into(), "A".into()]);
        assert!(r.iter().flatten().all(|(_, c)| c.len() <= SLOTS));
        b.left = vec!["KeyZ".into(), "KeyQ".into()];
        assert_eq!(rows(&b)[0][0].1, ["Z", "/", "Q", "/", "Num 4"].map(String::from));
        assert_eq!((pretty("KeyA").as_str(), pretty("Digit3").as_str(), pretty("Numpad7").as_str(), pretty("ArrowUp").as_str()), ("A", "3", "Num 7", "↑"));
    }

    #[test]
    fn pads_are_told_apart_by_vendor_or_name() {
        assert_eq!(pad_kind(Some(0x054C), "x"), Some(PadKind::PlayStation));
        assert_eq!(pad_kind(Some(0x045E), "x"), Some(PadKind::Xbox));
        assert_eq!(pad_kind(None, "Wireless Controller"), Some(PadKind::PlayStation));
        assert_eq!(pad_kind(None, "PS4 Controller"), Some(PadKind::PlayStation));
        assert_eq!(pad_kind(None, "Xbox Series X Controller"), Some(PadKind::Xbox));
        assert_eq!(pad_kind(None, "Some Generic Pad"), None);
    }

    #[test]
    fn leader_lines_end_on_the_outline() {
        // a circle of radius 5 at (50, 20): from the left at its height the line stops at its left edge
        assert_eq!(leader(Shape::Circle(50.0, 20.0, 5.0), true, 20.0), (45.0, 20.0));
        assert_eq!(leader(Shape::Circle(50.0, 20.0, 5.0), false, 20.0), (55.0, 20.0));
        // above or below it goes to the middle of the top / bottom edge
        assert_eq!(leader(Shape::Circle(50.0, 20.0, 5.0), true, 5.0), (50.0, 15.0));
        assert_eq!(leader(Shape::Circle(50.0, 20.0, 5.0), true, 40.0), (50.0, 25.0));
        // a rectangle: side facing the callout at its height, else its top / bottom edge
        assert_eq!(leader(Shape::Rect(10.0, 10.0, 20.0, 6.0), true, 12.0), (10.0, 12.0));
        assert_eq!(leader(Shape::Rect(10.0, 10.0, 20.0, 6.0), false, 12.0), (30.0, 12.0));
        assert_eq!(leader(Shape::Rect(10.0, 10.0, 20.0, 6.0), true, 2.0), (20.0, 10.0));
    }

    #[test]
    fn every_pad_has_its_marks() {
        for (ps, l) in [false, true].into_iter().flat_map(|ps| [PadLayout::Modern, PadLayout::Combat, PadLayout::Classic].map(move |l| (ps, l))) {
            let c = marks(ps, l);
            assert_eq!(c.iter().filter(|c| c.left).count(), 4);
            assert_eq!(c.iter().filter(|c| !c.left).count(), if l == PadLayout::Modern { 6 } else { 5 });
            assert!(c.iter().all(|c| {
                let (x, y) = c.shape.centre();
                x > 0.0 && x < 606.0 && y > 0.0 && y < 405.0
            }));
        }
    }
}
