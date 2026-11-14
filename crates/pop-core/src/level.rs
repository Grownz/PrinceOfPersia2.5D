//! Level resource (LEVELS.DAT; 2305 bytes, ids 2000 + level number).
//! Layout (verified against the shipped data, see tests):
//!   0    fg[720]        tile type (low 5 bits) + flag bits (high 3)
//!   720  bg[720]        tile modifier (door-link index for buttons, gate state, potion kind, ...)
//!   1440 doorlinks1[256], 1696 doorlinks2[256]
//!   1952 room links [24][4]
//!   2112 start room, start tile, start dir (0 = right, 255 = left), 4 more bytes (unparsed)
//!   2119 guard tile[24] (30 = none), 2143 guard dir[24]; remaining guard fields (skill, colour, ...) unparsed
pub const ROOMS: usize = 24;
pub const COLS: usize = 10;
pub const ROWS: usize = 3;
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
    Empty = 0, Floor, Spikes, Pillar, Gate, StuckButton, DropButton, TapestryFloor,
    BigPillarBottom, BigPillarTop, Potion, LooseFloor, TapestryTop, Mirror, Debris, RaiseButton,
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
}

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
                l.guards.push(Guard { room: r as u8 + 1, tile, dir: b[GUARD_DIR + r] });
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
