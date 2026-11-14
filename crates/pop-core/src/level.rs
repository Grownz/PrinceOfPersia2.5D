//! Level resource (LEVELS.DAT), layout as in the original game (2305 bytes).
pub const ROOMS: usize = 24;
pub const COLS: usize = 10;
pub const ROWS: usize = 3;
pub const TILES_PER_ROOM: usize = COLS * ROWS;
pub const LEVEL_SIZE: usize = 2305;

pub struct Level {
    /// foreground tile bytes: low 5 bits = tile type, high 3 bits = variant. `ROOMS * TILES_PER_ROOM`
    pub fg: [u8; ROOMS * TILES_PER_ROOM],
    pub bg: [u8; ROOMS * TILES_PER_ROOM],
    /// neighbours per room: [left, right, up, down], 1-based room numbers, 0 = none
    pub links: [[u8; 4]; ROOMS],
    pub start_room: u8,
    pub start_pos: u8,
    pub start_dir: i8,
}

impl Level {
    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() < LEVEL_SIZE {
            return None;
        }
        let n = ROOMS * TILES_PER_ROOM;
        let mut l = Level {
            fg: b[..n].try_into().ok()?,
            bg: b[n..2 * n].try_into().ok()?,
            links: [[0; 4]; ROOMS],
            start_room: b[2 * n + 512 + 96 + 25 + 24 + 24 + 15],
            start_pos: b[2 * n + 512 + 96 + 25 + 24 + 24 + 16],
            start_dir: b[2 * n + 512 + 96 + 25 + 24 + 24 + 17] as i8,
        };
        for (r, link) in l.links.iter_mut().enumerate() {
            link.copy_from_slice(&b[2 * n + 512 + r * 4..][..4]);
        }
        Some(l)
    }

    /// (tile type, variant) of tile `col,row` in 1-based `room`.
    pub fn tile(&self, room: usize, col: usize, row: usize) -> (u8, u8) {
        let t = self.fg[(room - 1) * TILES_PER_ROOM + row * COLS + col];
        (t & 0x1f, t >> 5)
    }
}
