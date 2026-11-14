//! Reader for Prince of Persia `.DAT` archives.
//! Layout: u32 index offset, then u16 index size; at the index offset: u16 count, count * {u16 id, u32 offset, u16 size}.
//! Each resource starts with a 1-byte checksum (compressed images additionally have a type byte; handled later).
use std::{collections::BTreeMap, fs, io, path::Path};

pub struct Dat {
    data: Vec<u8>,
    /// resource id -> (offset, size) of the payload (checksum byte excluded)
    index: BTreeMap<u16, (usize, usize)>,
}

fn u16le(b: &[u8], o: usize) -> io::Result<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(bad)
}
fn u32le(b: &[u8], o: usize) -> io::Result<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(bad)
}
fn bad() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "corrupt DAT file")
}

impl Dat {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_bytes(fs::read(path)?)
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let idx = u32le(&data, 0)? as usize;
        let count = u16le(&data, idx)? as usize;
        let mut index = BTreeMap::new();
        for i in 0..count {
            let e = idx + 2 + i * 8;
            let (id, off, size) = (u16le(&data, e)?, u32le(&data, e + 2)? as usize, u16le(&data, e + 6)? as usize);
            // size includes the checksum byte
            if size == 0 || off + size > data.len() {
                return Err(bad());
            }
            index.insert(id, (off + 1, size - 1));
        }
        Ok(Self { data, index })
    }

    pub fn ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.index.keys().copied()
    }

    pub fn get(&self, id: u16) -> Option<&[u8]> {
        self.index.get(&id).map(|&(o, s)| &self.data[o..o + s])
    }
}
