//! Minimap (option `minimap`, F8): a panel with the rooms the prince has entered, drawn from the level's own tiles. Each visited room is
//! composed once from the original tile images (VDUNGEON / VPALACE, the same pieces `tiles.rs` puts on the 3D rooms, flat as in the 2D
//! game), shrunk by two and shown as a UI image, so the GPU scales it with the panel. Rooms are placed on a grid by walking the room
//! links from the start room; rooms not entered yet are not drawn at all. Marks (UI shapes): potions (colour by kind, gone once drunk),
//! guards at their start tile, the entrance (green) and exit (gold) doors, and the prince. The view follows the prince.
//! Mouse: drag the panel to move it, drag its bottom right corner to resize it, wheel over it to zoom the map; the place, size and
//! zoom are kept in settings.json (`minimap_view`).
//! The map is 75 % opaque and eases to 25 % over a second while the prince is drawn behind it; a room is redrawn when a loose floor
//! in it has fallen. During a cinematic fight (sword drawn against a living guard) the whole map, frame included, fades out over a
//! second and back in afterwards. The default place is the bottom right, above the status bar.
//! Limits: a room shows the tiles as they were when first entered (no gate animation), torches have no flame, and the
//! guards are marked at their start tile only.
use crate::flow::Screen;
use crate::settings::Settings;
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use pop_core::level::{Level, Tile, COLS, ROOMS, ROWS, TILE_H_PX};
use pop_core::{dat::Dat, image};
use std::collections::{HashMap, HashSet, VecDeque};

/// A tile cell in the original's pixels, a composed room (10 x 3 cells, one spare row so it halves evenly) and its shrunk image.
const CELL_W: usize = 32;
const FULL_W: usize = COLS * CELL_W;
const FULL_H: usize = ROWS * TILE_H_PX + 1;
const IMG_W: usize = FULL_W / 2;
const IMG_H: usize = FULL_H / 2;

/// Height of the status bar along the bottom edge (flow.rs), px.
const STATUS_BAR: f32 = 28.0;

type Grid =[Option<(i32, i32)>; ROOMS + 1];

/// Grid cell (x right, y down) of every room, found from the links starting at the start room (index = 1-based room; None = not
/// reachable either way). A cell is taken by the first room that reaches it, links that disagree with that are ignored.
pub fn layout(links: &[[u8; 4]; ROOMS], start: usize) -> Grid {
    const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let mut pos: Grid = [None; ROOMS + 1];
    if !(1..=ROOMS).contains(&start) {
        return pos;
    }
    pos[start] = Some((0, 0));
    let mut taken = HashSet::from([(0, 0)]);
    let mut queue = VecDeque::from([start]);
    while let Some(r) = queue.pop_front() {
        let (x, y) = pos[r].unwrap_or_default();
        for (d, (dx, dy)) in DIRS.into_iter().enumerate() {
            // r's own links, and the rooms that link to r from the other side (one-way links)
            let mut found: Vec<(usize, (i32, i32))> = vec![(links[r - 1][d] as usize, (x + dx, y + dy))];
            for n in 1..=ROOMS {
                if links[n - 1][d ^ 1] as usize == r {
                    found.push((n, (x + dx, y + dy)));
                }
            }
            for (n, cell) in found {
                if (1..=ROOMS).contains(&n) && pos[n].is_none() && taken.insert(cell) {
                    pos[n] = Some(cell);
                    queue.push_back(n);
                }
            }
        }
    }
    pos
}

/// The decoded images and palettes of one tile set.
struct Art {
    imgs: HashMap<u16, image::Image>,
    /// the palace wall's speckle strips (363..377, one bit per pixel)
    strips: HashMap<u16, image::Image>,
    p200: Vec<[u8; 3]>,
    p360: Vec<[u8; 3]>,
}

impl Art {
    fn load(dir: &str, palace: bool) -> Option<Art> {
        let dat = Dat::open(format!("{dir}/{}", if palace { "VPALACE.DAT" } else { "VDUNGEON.DAT" })).ok()?;
        let mut all: HashMap<u16, image::Image> = dat.ids().filter_map(|id| Some((id, image::decode(dat.get(id)?)?))).collect();
        let strips = if palace { (363..=377).filter_map(|id| Some((id, all.remove(&id)?))).collect() } else { HashMap::new() };
        all.retain(|id, im| im.bpp == 4 && *id != 200 && *id != 360);
        let pal = |id: u16| dat.get(id).and_then(image::palette).unwrap_or_default();
        Some(Art { imgs: all, strips, p200: pal(200), p360: pal(360) })
    }

    /// Chtab 7 (361..377: the dungeon wall faces and marks) uses palette 360, everything else palette 200.
    fn pal(&self, id: u16) -> &[[u8; 3]] {
        if (361..=377).contains(&id) { &self.p360 } else { &self.p200 }
    }
}

/// Palette-indexed pixels (index 0 transparent) onto an RGB canvas at `x`, `y` (clipped).
fn blit(cv: &mut [[u8; 3]], (w, h): (usize, usize), px: &[u8], pal: &[[u8; 3]], x: i32, y: i32) {
    for sy in 0..h {
        for sx in 0..w {
            let p = px[sy * w + sx] as usize;
            let (dx, dy) = (x + sx as i32, y + sy as i32);
            if p != 0 && (0..FULL_W as i32).contains(&dx) && (0..FULL_H as i32).contains(&dy) {
                cv[dy as usize * FULL_W + dx as usize] = pal.get(p).copied().unwrap_or([255, 0, 255]);
            }
        }
    }
}

/// One room as RGBA, `IMG_W` x `IMG_H`: its tiles flat as in the 2D game, shrunk by 2 with a box filter.
fn compose(level: &Level, art: &Art, palace: bool, room: usize) -> Vec<u8> {
    use crate::tiles::{self, Role};
    let mut cv = vec![[0u8; 3]; FULL_W * FULL_H];
    let present = |id: u16| art.imgs.contains_key(&id);
    let pat = tiles::palace_pattern(room);
    // an image bottom-aligned on a cell and centred on it
    let put = |cv: &mut Vec<[u8; 3]>, id: u16, x0: i32, bottom: i32, above: i32| {
        if let Some(im) = art.imgs.get(&id) {
            blit(cv, (im.width, im.height), &im.pixels, art.pal(id), x0 + (CELL_W as i32 - im.width as i32) / 2, bottom - above - im.height as i32);
            im.height as i32
        } else {
            0
        }
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            let t = level.tile(room, col, row);
            let (x0, bottom) = ((col * CELL_W) as i32, ((row + 1) * TILE_H_PX) as i32);
            if t.kind == Tile::Wall {
                if palace {
                    let px = tiles::palace_wall(room, row, col, &pat, |id| art.strips.get(&id));
                    let mut pal = art.p360.clone();
                    pal.resize(tiles::MONO as usize + 1, [0; 3]);
                    pal[tiles::MONO as usize] = art.p200.get(6).copied().unwrap_or_default();
                    blit(&mut cv, tiles::PALACE_WALL, &px, &pal, x0, bottom - TILE_H_PX as i32);
                } else {
                    let is_wall = |c: i32| level.tile_wrapped(room as i32, c, row as i32) == Tile::Wall;
                    let m = tiles::wall_modifier(is_wall(col as i32 - 1), is_wall(col as i32 + 1)) as usize & 3;
                    for (id, y) in [(tiles::WALL_FACE[m], bottom - TILE_H_PX as i32), (tiles::WALL_STRIP[m], bottom - 3)] {
                        if let Some(im) = art.imgs.get(&id) {
                            blit(&mut cv, (im.width, im.height), &im.pixels, art.pal(id), x0, y);
                        }
                    }
                    for mk in tiles::dungeon_wall_marks(room, row, col, m as u8) {
                        if let Some(im) = art.imgs.get(&mk.id) {
                            blit(&mut cv, (im.width, im.height), &im.pixels, art.pal(mk.id), x0 + mk.x, bottom - mk.up - im.height as i32);
                        }
                    }
                }
                continue;
            }
            // a loose floor has no record images: it is drawn as a plain floor
            let kind = if t.kind == Tile::LooseFloor { Tile::Floor } else { t.kind };
            for (role, id) in tiles::pieces(kind, t.modifier, palace, present) {
                if matches!(role, Role::BackPanel | Role::JawBack | Role::SpikeBack | Role::TorchStub) {
                    put(&mut cv, id, x0, bottom, 0);
                }
            }
            let (floor, extra) = tiles::tile_images(kind, t.modifier, present);
            if let Some([b, r, l]) = floor {
                if let (Some(b), Some(r), Some(l)) = (art.imgs.get(&b), art.imgs.get(&r), art.imgs.get(&l)) {
                    let (w, h, px) = tiles::floor_pixels(b, r, l);
                    blit(&mut cv, (w, h), &px, &art.p200, x0, bottom - h as i32);
                }
            }
            if let Some(id) = extra {
                let h = put(&mut cv, id, x0, bottom, 0);
                if matches!(kind, Tile::ExitLeft | Tile::ExitRight) {
                    put(&mut cv, id + 1, x0, bottom, h); // the lintel on top
                }
            }
        }
    }
    shrink(&cv)
}

/// The canvas halved with a box filter, as opaque RGBA.
fn shrink(cv: &[[u8; 3]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(IMG_W * IMG_H * 4);
    for y in 0..IMG_H {
        for x in 0..IMG_W {
            let mut s = [0u32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = cv[(y * 2 + dy) * FULL_W + x * 2 + dx];
                (0..3).for_each(|c| s[c] += p[c] as u32);
            }
            out.extend([(s[0] / 4) as u8, (s[1] / 4) as u8, (s[2] / 4) as u8, 255]);
        }
    }
    out
}

/// Panel drag in progress.
#[derive(Default, Clone, Copy, PartialEq)]
enum Drag {
    #[default]
    None,
    /// moving; the cursor's offset from the panel's corner
    Move(Vec2),
    Size,
}

#[derive(Resource, Default)]
pub struct Minimap {
    /// the level the visited set and the room images belong to
    level: Option<usize>,
    /// bit r = room r (1-based) was entered
    visited: u32,
    grid: Option<Grid>,
    arts: [Option<Option<Art>>; 2],
    panel: Option<Entity>,
    content: Option<Entity>,
    kid: Option<Entity>,
    cells: [Option<Entity>; ROOMS + 1],
    /// potion marks (entity, room, tile): hidden once the potion is gone
    potions: Vec<(Entity, usize, usize)>,
    /// [left, top, width, height, room width] while in use (settings.minimap_view once committed)
    view: [f32; 5],
    init: bool,
    drag: Drag,
    dirty: bool,
    /// the map point in the middle of the panel, in room widths / heights from the grid origin
    focus: Vec2,
    snap: bool,
    /// room images and the loose floors each was drawn with (bit = tile): a fallen floor redraws its room
    handles: [Option<Handle<Image>>; ROOMS + 1],
    loose: [u32; ROOMS + 1],
    /// 0..1: how far the map has faded out because the prince is behind it (eased into the opacity)
    hide: f32,
    /// 0..1: how far the whole map (frame too) has faded out for a cinematic fight
    gone: f32,
    /// the [map opacity, frame opacity] last written to the nodes (-1 = write again)
    applied: [f32; 2],
}

/// Opacity of the map inside the frame, and when the prince stands behind it; it fades over `FADE_SECS`.
const OPACITY: f32 = 0.75;
const OPACITY_BEHIND: f32 = 0.25;
const FADE_SECS: f32 = 1.0;

/// The colours a node has at full opacity; the fade scales their alpha. A `frame` node (border, grip) is not made translucent
/// like the map, it only fades out for a cinematic fight.
#[derive(Component)]
pub(crate) struct Fade {
    bg: Color,
    border: Option<Color>,
    frame: bool,
}

/// Which of the room's tiles are loose floors.
fn loose_mask(lv: &Level, room: usize) -> u32 {
    (0..COLS * ROWS).fold(0, |m, i| m | ((lv.tile_at(room, i).kind == Tile::LooseFloor) as u32) << i)
}

/// The panel with its map layer, the prince's mark and the resize grip.
pub fn setup(mut cmds: Commands, mut mm: ResMut<Minimap>) {
    let gold = Color::srgb(0.85, 0.7, 0.35);
    let absolute = || Node { position_type: PositionType::Absolute, ..default() };
    let panel = cmds
        .spawn((Node { left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(300.0), height: Val::Px(190.0), overflow: Overflow::clip(), border: UiRect::all(Val::Px(2.0)), ..absolute() }, BackgroundColor(Color::NONE), BorderColor::all(gold), Fade { bg: Color::NONE, border: Some(gold), frame: true }, GlobalZIndex(8), Visibility::Hidden))
        .id();
    // the dark backdrop is a layer of its own: it is translucent like the map, the border is not
    let back = cmds.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..absolute() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)), Fade { bg: Color::srgba(0.0, 0.0, 0.0, 0.72), border: None, frame: false })).id();
    let content = cmds.spawn(absolute()).id();
    let kid = cmds
        .spawn((Node { width: Val::Px(10.0), height: Val::Px(10.0), border: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..absolute() }, BackgroundColor(Color::WHITE), BorderColor::all(Color::srgb(0.1, 0.45, 1.0)), Fade { bg: Color::WHITE, border: Some(Color::srgb(0.1, 0.45, 1.0)), frame: false }, ZIndex(10)))
        .id();
    let grip = cmds.spawn((Node { right: Val::Px(0.0), bottom: Val::Px(0.0), width: Val::Px(14.0), height: Val::Px(14.0), ..absolute() }, BackgroundColor(gold), Fade { bg: gold, border: None, frame: true }, ZIndex(11))).id();
    cmds.entity(content).add_child(kid);
    cmds.entity(panel).add_children(&[back, content, grip]);
    (mm.panel, mm.content, mm.kid) = (Some(panel), Some(content), Some(kid));
}

/// F8 switches the map.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<Screen>>, mut set: ResMut<Settings>) {
    if keys.just_pressed(KeyCode::F8) && matches!(screen.get(), Screen::Playing | Screen::Paused) {
        set.minimap = !set.minimap;
        set.save();
    }
}

fn rect(v: &[f32; 5], win: Vec2) -> Rect {
    let (w, h) = (v[2].min(win.x), v[3].min(win.y));
    let left = if v[0] < 0.0 { win.x - w - 16.0 } else { v[0] }.clamp(0.0, (win.x - w).max(0.0));
    let top = if v[1] < 0.0 { win.y - h - STATUS_BAR - 16.0 } else { v[1] }.clamp(0.0, (win.y - h).max(0.0));
    Rect::new(left, top, left + w, top + h)
}

fn potion_colour(kind: u8) -> Color {
    match crate::items::potion_look(kind).1 {
        Some(0) => Color::srgb(0.95, 0.15, 0.15),
        Some(1) => Color::srgb(0.2, 0.85, 0.3),
        Some(_) => Color::srgb(0.3, 0.5, 1.0),
        None => Color::srgb(0.6, 0.6, 0.6),
    }
}

/// A mark in a room: centre `x`, `y` and width `w` in percent of the room, `aspect` = width / height.
fn mark<'a>(p: &'a mut ChildSpawnerCommands<'_>, (x, y, w): (f32, f32, f32), aspect: f32, fill: Color, round: bool) -> EntityCommands<'a> {
    let h = w / aspect / (IMG_H as f32 / IMG_W as f32); // percent of the room's height
    p.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Percent(x - w / 2.0), top: Val::Percent(y - h / 2.0), width: Val::Percent(w), aspect_ratio: Some(aspect), border: UiRect::all(Val::Px(1.0)), border_radius: if round { BorderRadius::MAX } else { BorderRadius::ZERO }, ..default() },
        BackgroundColor(fill),
        BorderColor::all(Color::BLACK),
        Fade { bg: fill, border: Some(Color::BLACK), frame: false },
    ))
}

/// Notes the entered rooms, resets for a new level, and builds the image and marks of a room the first time it is shown.
#[allow(clippy::too_many_arguments)]
pub fn track(mut cmds: Commands, g: Res<Game>, set: Res<Settings>, screen: Res<State<Screen>>, mut mm: ResMut<Minimap>, mut imgs: ResMut<Assets<Image>>) {
    if mm.level != Some(g.level) {
        for e in mm.cells.iter().flatten() {
            cmds.entity(*e).despawn();
        }
        let lv = &g.world.level;
        (mm.level, mm.visited, mm.cells, mm.potions, mm.snap) = (Some(g.level), 0, [None; ROOMS + 1], vec![], true);
        (mm.handles, mm.loose) = (Default::default(), [0; ROOMS + 1]);
        mm.grid = Some(layout(&lv.links, lv.start_room as usize));
    }
    if *screen.get() == Screen::Playing && (1..=ROOMS).contains(&g.room) {
        mm.visited |= 1 << g.room;
    }
    let (Some(content), Some(grid), true) = (mm.content, mm.grid, set.minimap) else { return };
    let todo: Vec<usize> = (1..=ROOMS).filter(|&r| mm.visited >> r & 1 == 1 && mm.cells[r].is_none() && grid[r].is_some()).collect();
    // a loose floor that fell (or came back with a restart) changes its room's picture
    let stale: Vec<usize> = (1..=ROOMS).filter(|&r| mm.handles[r].is_some() && mm.loose[r] != loose_mask(&g.world.level, r)).collect();
    if todo.is_empty() && stale.is_empty() {
        return;
    }
    let palace = crate::kid::palace(g.level);
    if mm.arts[palace as usize].is_none() {
        mm.arts[palace as usize] = Some(Art::load(&crate::data_dir(), palace));
    }
    let Some(Some(art)) = &mm.arts[palace as usize] else { return };
    let lv = &g.world.level;
    let built: Vec<(usize, Vec<u8>)> = todo.iter().chain(&stale).map(|&r| (r, compose(lv, art, palace, r))).collect();
    let (new, old) = built.split_at(todo.len());
    for (r, rgba) in old {
        if let Some(mut img) = mm.handles[*r].as_ref().and_then(|h| imgs.get_mut(h)) {
            img.data = Some(rgba.clone());
        }
        mm.loose[*r] = loose_mask(lv, *r);
    }
    mm.applied = [-1.0; 2]; // the new nodes get the current opacity
    let start = (lv.start_room as usize, lv.start_tile as usize);
    for (r, rgba) in new.iter().cloned() {
        let mut img = Image::new(Extent3d { width: IMG_W as u32, height: IMG_H as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        img.sampler = ImageSampler::linear();
        let handle = imgs.add(img);
        (mm.handles[r], mm.loose[r]) = (Some(handle.clone()), loose_mask(lv, r));
        let mut potions = vec![];
        let cell = cmds
            .spawn((ImageNode::new(handle), Node { position_type: PositionType::Absolute, ..default() }))
            .with_children(|p| {
                let at = |tile: usize, dy: f32| ((tile % COLS) as f32 * 10.0 + 5.0, ((tile / COLS) as f32 + dy) / ROWS as f32 * 100.0);
                for tile in 0..COLS * ROWS {
                    let t = lv.tile_at(r, tile);
                    match t.kind {
                        Tile::Potion => {
                            let (x, y) = at(tile, 0.8);
                            let e = mark(p, (x, y, 4.0), 1.0, potion_colour(t.modifier >> 5), true).id();
                            potions.push((e, r, tile));
                        }
                        Tile::ExitLeft => {
                            // the level's start door is the entrance (green), any other door an exit (gold)
                            let entrance = r == start.0 && start.1 / COLS == tile / COLS && (start.1 % COLS).abs_diff(tile % COLS) <= 2;
                            let (x, y) = at(tile, 0.55);
                            mark(p, (x + 5.0, y, 5.0), 0.6, if entrance { Color::srgb(0.2, 0.9, 0.5) } else { Color::srgb(1.0, 0.8, 0.2) }, false);
                        }
                        _ => {}
                    }
                }
                for gd in lv.guards.iter().filter(|gd| gd.room as usize == r) {
                    let (x, y) = at(gd.tile as usize, 0.55);
                    mark(p, (x, y, 4.5), 1.0, Color::srgb(0.85, 0.1, 0.1), false);
                }
            })
            .id();
        cmds.entity(content).add_child(cell);
        mm.cells[r] = Some(cell);
        mm.potions.extend(potions);
    }
}

/// Moves, resizes and zooms the panel with the mouse; the result goes into settings.json once the mouse is let go.
pub fn interact(mb: Res<ButtonInput<MouseButton>>, mut wheel: MessageReader<MouseWheel>, windows: Query<&Window, With<PrimaryWindow>>, screen: Res<State<Screen>>, mut set: ResMut<Settings>, mut mm: ResMut<Minimap>) {
    let dy: f32 = wheel.read().map(|e| e.y).sum(); // always drain
    if !mm.init {
        (mm.view, mm.init) = (set.minimap_view, true);
    }
    let (Ok(win), true) = (windows.single(), *screen.get() == Screen::Playing && set.minimap && mm.gone < 1.0) else {
        mm.drag = Drag::None;
        return;
    };
    let (cur, size) = (win.cursor_position(), Vec2::new(win.width(), win.height()));
    let r = rect(&mm.view, size);
    if let Some(c) = cur.filter(|c| r.contains(*c)) {
        if mb.just_pressed(MouseButton::Left) {
            mm.drag = if c.x > r.max.x - 18.0 && c.y > r.max.y - 18.0 { Drag::Size } else { Drag::Move(c - r.min) };
        }
        if dy != 0.0 {
            mm.view[4] = (mm.view[4] * 1.12f32.powf(dy)).clamp(30.0, 900.0);
            mm.dirty = true;
        }
    }
    let held = mb.pressed(MouseButton::Left);
    match (mm.drag, cur) {
        (Drag::Move(off), Some(c)) if held => {
            mm.view[0] = (c.x - off.x).max(0.0);
            mm.view[1] = (c.y - off.y).max(0.0);
        }
        (Drag::Size, Some(c)) if held => {
            mm.view[2] = (c.x - r.min.x).clamp(60.0, size.x);
            mm.view[3] = (c.y - r.min.y).clamp(40.0, size.y);
        }
        _ => {}
    }
    if !held {
        mm.dirty |= mm.drag != Drag::None;
        mm.drag = Drag::None;
    }
    if mm.dirty && !held && dy == 0.0 {
        mm.dirty = false;
        let s = set.bypass_change_detection(); // only the saved file changes, nothing reacts to it
        s.minimap_view = mm.view;
        s.save();
    }
}

/// Places the panel, the rooms and the marks, and eases the view after the prince.
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_ui(
    time: Res<Time>,
    g: Res<Game>,
    set: Res<Settings>,
    screen: Res<State<Screen>>,
    gore: Res<crate::blood::Gore>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    view: Res<crate::camera::View>,
    mut mm: ResMut<Minimap>,
    mut nodes: Query<&mut Node>,
    mut vis: Query<&mut Visibility>,
    mut fades: Query<(&Fade, &mut BackgroundColor, &mut BorderColor)>,
    mut room_imgs: Query<&mut ImageNode>,
) {
    let (Some(panel), Some(content), Some(kid), Some(grid)) = (mm.panel, mm.content, mm.kid, mm.grid) else { return };
    // a cinematic fight fades the whole map out (frame included), and back in when it is over
    mm.gone = (mm.gone + time.delta_secs() / FADE_SECS * if view.fighting { 1.0 } else { -1.0 }).clamp(0.0, 1.0);
    let show = set.minimap && matches!(screen.get(), Screen::Playing | Screen::Paused) && mm.cells.iter().any(|c| c.is_some()) && mm.gone < 1.0;
    if let Ok(mut v) = vis.get_mut(panel) {
        *v = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    let Ok(win) = windows.single() else { return };
    if !show {
        return;
    }
    let r = rect(&mm.view, Vec2::new(win.width(), win.height()));
    // the map fades out while the prince is drawn behind it (his middle, at the figures' depth 0.25)
    let behind = *screen.get() == Screen::Playing
        && gore.bodies[0]
            .and_then(|b| {
                let (cam, tf) = cams.single().ok()?;
                let p = cam.world_to_viewport(tf, Vec3::new(b.x, b.y + crate::room::TILE_H / 2.0, 0.25)).ok()?;
                Some(if g.world.upside { Vec2::new(p.x, win.height() - p.y) } else { p })
            })
            .is_some_and(|p| r.contains(p));
    mm.hide = (mm.hide + time.delta_secs() / FADE_SECS * if behind { 1.0 } else { -1.0 }).clamp(0.0, 1.0);
    let ease = mm.hide * mm.hide * (3.0 - 2.0 * mm.hide);
    let frame = 1.0 - mm.gone * mm.gone * (3.0 - 2.0 * mm.gone); // the fight fade, eased
    let op = (OPACITY + (OPACITY_BEHIND - OPACITY) * ease) * frame;
    if (op - mm.applied[0]).abs() > 1e-4 || (frame - mm.applied[1]).abs() > 1e-4 {
        mm.applied = [op, frame];
        for (f, mut bg, mut bc) in &mut fades {
            let k = if f.frame { frame } else { op };
            bg.0 = f.bg.with_alpha(f.bg.alpha() * k);
            if let Some(b) = f.border {
                *bc = BorderColor::all(b.with_alpha(k));
            }
        }
        for mut im in &mut room_imgs {
            im.color = Color::srgba(1.0, 1.0, 1.0, op);
        }
    }
    let (rw, rh) = (mm.view[4], mm.view[4] * IMG_H as f32 / IMG_W as f32);
    if let Ok(mut n) = nodes.get_mut(panel) {
        (n.left, n.top, n.width, n.height) = (Val::Px(r.min.x), Val::Px(r.min.y), Val::Px(r.width()), Val::Px(r.height()));
    }
    for (cell, pos) in mm.cells.iter().zip(grid.iter()).skip(1) {
        if let (Some(e), Some((gx, gy))) = (*cell, *pos) {
            if let Ok(mut n) = nodes.get_mut(e) {
                (n.left, n.top, n.width, n.height) = (Val::Px(gx as f32 * rw), Val::Px(gy as f32 * rh), Val::Px(rw), Val::Px(rh));
            }
        }
    }
    // the prince inside the shown room: x in room widths, y in room heights (his feet are at world y = 0 on the bottom row)
    let (fx, fy) = gore.bodies[0].map_or((0.5, 0.5), |b| ((b.x / COLS as f32).clamp(0.0, 1.0), ((3.0 - b.y / crate::room::TILE_H - 0.35) / ROWS as f32).clamp(0.0, 1.0)));
    if let (Some((gx, gy)), Ok(mut n)) = (grid[g.room.min(ROOMS)], nodes.get_mut(kid)) {
        let target = Vec2::new(gx as f32 + fx, gy as f32 + fy);
        mm.focus = if mm.snap { target } else { mm.focus + (target - mm.focus) * (1.0 - (-5.0 * time.delta_secs()).exp()) };
        mm.snap = false;
        (n.left, n.top) = (Val::Px(target.x * rw - 5.0), Val::Px(target.y * rh - 5.0));
    }
    if let Ok(mut n) = nodes.get_mut(content) {
        (n.left, n.top) = (Val::Px(r.width() / 2.0 - mm.focus.x * rw), Val::Px(r.height() / 2.0 - mm.focus.y * rh));
    }
    for &(e, room, tile) in &mm.potions {
        if let Ok(mut v) = vis.get_mut(e) {
            *v = if g.world.level.tile_at(room, tile).kind == Tile::Potion { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_are_placed_by_their_links_also_one_way() {
        let mut links = [[0u8; 4]; ROOMS];
        // 1 - 2 side by side, 3 below 2, 4 above 1 (only 4 links down to 1: a one-way link)
        links[0][1] = 2;
        links[1][0] = 1;
        links[1][3] = 3;
        links[3][3] = 1;
        let g = layout(&links, 1);
        assert_eq!((g[1], g[2], g[3], g[4]), (Some((0, 0)), Some((1, 0)), Some((1, 1)), Some((0, -1))));
        assert_eq!(g[5], None);
        assert_eq!(layout(&links, 0), [None; ROOMS + 1]);
    }

    #[test]
    fn a_room_that_fills_a_taken_cell_is_dropped_not_stacked() {
        let mut links = [[0u8; 4]; ROOMS];
        links[0][1] = 2; // 1 -> 2 right
        links[0][3] = 3; // 1 -> 3 below
        links[2][1] = 4; // 3 -> 4 right: (1, 1)
        links[1][3] = 5; // 2 -> 5 below: (1, 1) too
        let g = layout(&links, 1);
        let cells: Vec<_> = g.iter().flatten().collect();
        assert_eq!(cells.len(), cells.iter().collect::<HashSet<_>>().len(), "no two rooms share a cell");
    }

    #[test]
    fn shrinking_a_room_keeps_colours_and_gives_the_image_size() {
        let cv = vec![[200u8, 100, 0]; FULL_W * FULL_H];
        let s = shrink(&cv);
        assert_eq!((s.len(), &s[..4]), (IMG_W * IMG_H * 4, &[200u8, 100, 0, 255][..]));
    }

    #[test]
    fn the_panel_stays_in_the_window() {
        let win = Vec2::new(800.0, 600.0);
        assert_eq!(rect(&[-1.0, 16.0, 300.0, 190.0, 150.0], win).min, Vec2::new(484.0, 16.0));
        assert_eq!(rect(&[-1.0, -1.0, 300.0, 190.0, 150.0], win).min, Vec2::new(484.0, 366.0)); // 600 - 190 - 28 - 16
        let r = rect(&[790.0, 590.0, 300.0, 190.0, 150.0], win);
        assert!(r.max.x <= 800.0 && r.max.y <= 600.0);
        assert!(rect(&[0.0, 0.0, 900.0, 900.0, 150.0], win).width() <= 800.0);
    }
}
