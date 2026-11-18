//! Level resource (LEVELS.DAT; 2305 bytes, ids 2000 + level number). load_level (0000:0E97) reads 0x901 = 2305 bytes raw
//! to ds:50C2; who reads which byte (xrefs over PRINCE.EXE v1.1, docs/tiles.md 1.5):
//!   0    fg[720]        tile type (low 5 bits) + flag bits (high 3; only 0x20 is ever read: a loose floor that never falls)
//!   720  bg[720]        tile modifier (door-link index for buttons, gate state, potion kind, ...)
//!   1440 doorlinks1[256], 1696 doorlinks2[256]
//!   1952 room links [24][4]
//!   2048 used_rooms     only read by alter_mods_allrm (0000:B622): rooms 1..=used_rooms get load_alter_mod
//!   2049 room x[24], 2073 room y[24], 2097 15 fill bytes: editor data, never read by the game
//!   2112 start room, start tile, start dir (0 = right, 255 = left); 2115..2118 never read
//!   2119 guard tile[24] (30 = none), 2143 dir, 2167 x, 2191 seq lo, 2215 skill, 2239 seq hi, 2263 colour (docs/combat.md)
//!   2287 18 fill bytes, never read. Resource 2015 is 2304 bytes (its unread last fill byte is missing).
const USED_ROOMS: usize = 2048;
pub const ROOMS: usize = 24;
pub const COLS: usize = 10;
pub const ROWS: usize = 3;
/// height of one tile row on the original 320x200 screen
pub const TILE_H_PX: usize = 63;
pub const TILES_PER_ROOM: usize = COLS * ROWS;

const N: usize = ROOMS * TILES_PER_ROOM;
const DOORLINKS: usize = 2 * N;
const ROOMLINKS: usize = DOORLINKS + 512;
const START: usize = 2112;
const GUARD_TILE: usize = 2119;
const GUARD_DIR: usize = GUARD_TILE + ROOMS;
const MIN_LEN: usize = GUARD_DIR + ROOMS; // resource 2015 is 1 byte shorter than the rest

/// Tile types (low 5 bits of the fg byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tile {
    Empty = 0, Floor, Spikes, Pillar, Gate, StuckButton, DropButton, DoortopWithFloor,
    BigPillarBottom, BigPillarTop, Potion, LooseFloor, Doortop, Mirror, Debris, RaiseButton,
    ExitLeft, ExitRight, Chomper, Torch, Wall, Skeleton, Sword, BalconyLeft,
    BalconyRight, LatticePillar, LatticeSupport, SmallLattice, LatticeLeft, LatticeRight, TorchDebris,
    /// 31 occurs in the data but is unidentified.
    Unknown31,
}

impl Tile {
    pub fn from_u8(b: u8) -> Tile {
        // SAFETY: repr(u8) with contiguous discriminants 0..=31; masked to 5 bits.
        unsafe { std::mem::transmute(b & 0x1f) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRef {
    pub kind: Tile,
    /// high 3 bits of the fg byte (meaning depends on tile kind)
    pub flags: u8,
    /// bg byte
    pub modifier: u8,
}

/// Target of a button's door link: a tile in a (1-based) room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DoorLink {
    pub room: u8,
    pub tile: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guard {
    pub room: u8,
    pub tile: u8,
    /// 0 = facing right, 255 = facing left
    pub dir: u8,
    /// skill level 0..=11 (level tail offset 2215, docs/combat.md)
    pub skill: u8,
    /// colour/palette index (offset 2263)
    pub colour: u8,
}

#[derive(Clone)]
pub struct Level {
    pub fg: [u8; N],
    pub bg: [u8; N],
    doorlinks1: [u8; 256],
    doorlinks2: [u8; 256],
    /// neighbours per room: [left, right, up, down], 1-based room numbers, 0 = none
    pub links: [[u8; 4]; ROOMS],
    pub start_room: u8,
    pub start_tile: u8,
    /// 0 = facing right, 255 = facing left
    pub start_dir: u8,
    pub guards: Vec<Guard>,
}

impl Level {
    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() < MIN_LEN {
            return None;
        }
        // alter_mods_allrm (0000:B5FF): load_alter_mod (0000:B656, docs/tiles.md 1.2) for rooms 1..=used_rooms, raw -> runtime
        // modifiers. Room r's tiles are at 30*(r-1) and its modifiers 720 further on; get_room_address (0000:BAD4) has no upper
        // bound, so used_rooms 25 (most shipped levels) also alters a pseudo room 25 whose "tiles" are room 1's (already
        // altered) modifiers and whose "modifiers" are doorlinks1[0..30] (docs/tiles.md 1.5). Rooms above used_rooms keep raw bytes.
        let mut raw = b.to_vec();
        for room in 1..=b[USED_ROOMS] as usize {
            for i in 0..TILES_PER_ROOM {
                let (t, m) = (30 * (room - 1) + i, N + 30 * (room - 1) + i);
                if m >= DOORLINKS + 256 {
                    break; // shortcut: used_rooms > 25 would alter memory past doorlinks1 (never in the data)
                }
                raw[m] = match Tile::from_u8(raw[t]) {
                    Tile::Gate => if raw[m] == 1 { 188 } else { 0 }, // 188 = open, 0 = closed
                    Tile::LooseFloor => 0,
                    // level 15's copy-protection potion (kind 6, 0000:B6B7) is not modelled
                    Tile::Potion => raw[m] << 5,
                    // walls: (m << 7) | neighbour bits, drawing only; not modelled (no pseudo room-25 wall hits a used doorlink)
                    _ => raw[m],
                };
            }
        }
        let b = &raw[..];
        let mut l = Level {
            fg: b[..N].try_into().ok()?,
            bg: b[N..2 * N].try_into().ok()?,
            doorlinks1: b[DOORLINKS..DOORLINKS + 256].try_into().ok()?,
            doorlinks2: b[DOORLINKS + 256..DOORLINKS + 512].try_into().ok()?,
            links: [[0; 4]; ROOMS],
            start_room: b[START],
            start_tile: b[START + 1],
            start_dir: b[START + 2],
            guards: Vec::new(),
        };
        for (r, link) in l.links.iter_mut().enumerate() {
            link.copy_from_slice(&b[ROOMLINKS + r * 4..][..4]);
        }
        for r in 0..ROOMS {
            let tile = b[GUARD_TILE + r];
            if (tile as usize) < TILES_PER_ROOM {
                l.guards.push(Guard { room: r as u8 + 1, tile, dir: b[GUARD_DIR + r], skill: b.get(2215 + r).copied().unwrap_or(0), colour: b.get(2263 + r).copied().unwrap_or(0) });
            }
        }
        Some(l)
    }

    /// Tile `col,row` of 1-based `room`.
    pub fn tile(&self, room: usize, col: usize, row: usize) -> TileRef {
        self.tile_at(room, row * COLS + col)
    }

    pub fn tile_at(&self, room: usize, tile: usize) -> TileRef {
        let i = (room - 1) * TILES_PER_ROOM + tile;
        TileRef { kind: Tile::from_u8(self.fg[i]), flags: self.fg[i] >> 5, modifier: self.bg[i] }
    }

    /// Follow room links for out-of-range columns/rows; `None` = outside the level (room 0 counts as a wall).
    pub fn wrap(&self, mut room: i32, mut col: i32, mut row: i32) -> Option<(usize, usize)> {
        for _ in 0..8 {
            if !(1..=ROOMS as i32).contains(&room) {
                return None;
            }
            let l = &self.links[room as usize - 1];
            if col < 0 {
                room = l[0] as i32;
                col += COLS as i32;
            } else if col >= COLS as i32 {
                room = l[1] as i32;
                col -= COLS as i32;
            } else if row < 0 {
                room = l[2] as i32;
                row += ROWS as i32;
            } else if row >= ROWS as i32 {
                room = l[3] as i32;
                row -= ROWS as i32;
            } else {
                return Some((room as usize, row as usize * COLS + col as usize));
            }
        }
        None
    }

    /// Tile type at `col,row` of `room`, following room links. Room 0 (outside the level) is a wall, as in the original.
    pub fn tile_wrapped(&self, room: i32, col: i32, row: i32) -> Tile {
        self.wrap(room, col, row).map_or(Tile::Wall, |(r, i)| self.tile_at(r, i).kind)
    }

    /// Modifier byte of the tile (0 outside the level).
    pub fn modifier_wrapped(&self, room: i32, col: i32, row: i32) -> u8 {
        self.wrap(room, col, row).map_or(0, |(r, i)| self.tile_at(r, i).modifier)
    }

    /// Index into `fg`/`bg` of tile `idx` (0..30) of 1-based `room`.
    fn at(room: usize, idx: usize) -> usize {
        (room - 1) * TILES_PER_ROOM + idx
    }

    /// Runtime change of a tile type. The original writes whole bytes, which clears the 3 flag bits.
    pub fn set_tile(&mut self, room: usize, idx: usize, t: Tile) {
        self.fg[Self::at(room, idx)] = t as u8;
    }

    pub fn set_modifier(&mut self, room: usize, idx: usize, m: u8) {
        self.bg[Self::at(room, idx)] = m;
    }

    /// Animation frame of a torch, potion or sword from its runtime modifier, as the original draws it (docs/tiles.md 8.2-8.4):
    /// torch: flame 0..8 (PRINCE.DAT 151 + f, `0000:A5E1`; none if mod >= 9); potion: bubble frame mod & 0x0F (1..7 =
    /// PRINCE.DAT 166..172, 0 = not animated yet, `0000:A8AE`); sword: 1 while the glint image is shown (mod == 1, `0000:A910`).
    /// The modifiers advance in `sim::World` (process_trobs) only for the drawn room. None for other tiles.
    pub fn anim_frame(&self, room: usize, idx: usize) -> Option<u8> {
        let t = self.tile_at(room, idx);
        match t.kind {
            Tile::Torch | Tile::TorchDebris => (t.modifier < 9).then_some(t.modifier),
            Tile::Potion => Some(t.modifier & 0x0F),
            Tile::Sword => Some((t.modifier == 1) as u8),
            _ => None,
        }
    }

    /// Raw fg byte (type + flag bits); bit 0x20 on a loose floor means it never falls.
    pub fn fg_byte(&self, room: usize, idx: usize) -> u8 {
        self.fg[Self::at(room, idx)]
    }

    /// Button timer of a door-link entry (low 5 bits of doorlinks2, shared by all buttons with this link index).
    pub fn link_timer(&self, idx: u8) -> u8 {
        self.doorlinks2[idx as usize] & 0x1f
    }

    pub fn set_link_timer(&mut self, idx: u8, t: u8) {
        let d = &mut self.doorlinks2[idx as usize];
        *d = (*d & 0xe0) | (t & 0x1f);
    }

    /// Tiles a button (`modifier` = its link index) triggers: the chain runs until an entry with bit 7 set.
    pub fn door_links(&self, mut idx: u8) -> Vec<DoorLink> {
        let mut out = Vec::new();
        loop {
            let (a, c) = (self.doorlinks1[idx as usize], self.doorlinks2[idx as usize]);
            out.push(DoorLink { room: ((a & 0x60) >> 5) | ((c & 0xe0) >> 3), tile: a & 0x1f });
            if a & 0x80 != 0 || idx == 255 {
                return out;
            }
            idx += 1;
        }
    }
}
