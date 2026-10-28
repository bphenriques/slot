//! A pack: something for every known dump, ready made, in one file a user copies to the card so
//! slot needs no network for it. `Labels/Artwork.pack` holds cart labels as JPEG and
//! `Cheats/Cheats.pack` holds cheat lists as `.cht` text. The file is
//!
//! ```text
//! magic (8 bytes), entry count (u32)
//! per entry, sorted by CRC32: the ROM's CRC32, data offset, data length (u32 each)
//! the data, each distinct entry once
//! ```
//!
//! with every integer little-endian. A lookup reads the table and one entry, never the whole file.
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const FILE: &str = "Artwork.pack";
pub const ARTWORK: &[u8; 8] = b"SLOTART1";
pub const CHEATS: &[u8; 8] = b"SLOTCHT1";
const HEAD: u64 = 12;
const ENTRY: u64 = 12;
const MAX_ENTRY: u32 = 4 << 20;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

/// Writes a pack of (CRC32, data) entries. A checksum listed twice keeps its first data, and
/// dumps with the same data share one copy of it.
pub fn write(
    out: &mut impl Write,
    magic: &[u8; 8],
    mut entries: Vec<(u32, Vec<u8>)>,
) -> io::Result<()> {
    entries.sort_by_key(|(crc, _)| *crc);
    entries.dedup_by_key(|(crc, _)| *crc);
    let too_big = || io::Error::new(io::ErrorKind::InvalidInput, "pack over 4 GB");
    let count = u32::try_from(entries.len()).map_err(|_| too_big())?;
    out.write_all(magic)?;
    out.write_all(&count.to_le_bytes())?;
    let mut offset = HEAD + ENTRY * u64::from(count);
    let mut placed: HashMap<&[u8], u32> = HashMap::new();
    let mut data: Vec<&[u8]> = Vec::new();
    for (crc, bytes) in &entries {
        let len = u32::try_from(bytes.len()).map_err(|_| too_big())?;
        let at = match placed.get(bytes.as_slice()) {
            Some(at) => *at,
            None => {
                let at = u32::try_from(offset).map_err(|_| too_big())?;
                placed.insert(bytes, at);
                data.push(bytes);
                offset += u64::from(len);
                at
            }
        };
        for field in [*crc, at, len] {
            out.write_all(&field.to_le_bytes())?;
        }
    }
    u32::try_from(offset).map_err(|_| too_big())?;
    for bytes in data {
        out.write_all(bytes)?;
    }
    Ok(())
}

/// The pack's data for a ROM. No pack is `Ok(None)`; a damaged one, or one of another kind, is an
/// error.
pub(crate) fn find(path: &Path, magic: &[u8; 8], crc: u32) -> io::Result<Option<Vec<u8>>> {
    let mut file = match File::open(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        file => file?,
    };
    let size = file.metadata()?.len();
    let mut head = [0u8; HEAD as usize];
    file.read_exact(&mut head)
        .map_err(|_| invalid("pack too short"))?;
    if &head[..8] != magic {
        return Err(invalid("not this kind of pack"));
    }
    let count = u64::from(u32::from_le_bytes(head[8..12].try_into().unwrap()));
    if HEAD + ENTRY * count > size {
        return Err(invalid("pack table runs past the end"));
    }
    let mut table = vec![0u8; (ENTRY * count) as usize];
    file.read_exact(&mut table)?;
    let field =
        |entry: &[u8], i: usize| u32::from_le_bytes(entry[i * 4..i * 4 + 4].try_into().unwrap());
    let entries: Vec<&[u8]> = table.chunks_exact(ENTRY as usize).collect();
    let Ok(at) = entries.binary_search_by_key(&crc, |entry| field(entry, 0)) else {
        return Ok(None);
    };
    let (offset, len) = (field(entries[at], 1), field(entries[at], 2));
    if len > MAX_ENTRY || u64::from(offset) + u64::from(len) > size {
        return Err(invalid("pack entry runs past the end"));
    }
    let mut data = vec![0u8; len as usize];
    file.seek(SeekFrom::Start(offset.into()))?;
    file.read_exact(&mut data)?;
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_pack_finds_each_image_by_checksum_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut out = Vec::new();
        write(
            &mut out,
            ARTWORK,
            vec![
                (30, b"third".to_vec()),
                (10, b"first".to_vec()),
                (20, b"second".to_vec()),
                (10, b"duplicate".to_vec()),
            ],
        )
        .unwrap();
        std::fs::write(&path, &out).unwrap();
        for (crc, image) in [(10, &b"first"[..]), (20, b"second"), (30, b"third")] {
            assert_eq!(find(&path, ARTWORK, crc).unwrap().unwrap(), image);
        }
        assert_eq!(find(&path, ARTWORK, 15).unwrap(), None);
        assert_eq!(
            find(&dir.path().join("missing.pack"), ARTWORK, 10).unwrap(),
            None
        );
        assert!(find(&path, CHEATS, 10).is_err());
    }

    #[test]
    fn dumps_with_the_same_data_share_one_copy() {
        let shared = vec![b'x'; 1000];
        let mut out = Vec::new();
        write(
            &mut out,
            CHEATS,
            vec![
                (1, shared.clone()),
                (2, shared.clone()),
                (3, b"own".to_vec()),
            ],
        )
        .unwrap();
        assert_eq!(out.len(), 12 + 3 * 12 + 1000 + 3);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cheats.pack");
        std::fs::write(&path, &out).unwrap();
        for crc in [1, 2] {
            assert_eq!(find(&path, CHEATS, crc).unwrap().unwrap(), shared);
        }
        assert_eq!(find(&path, CHEATS, 3).unwrap().unwrap(), b"own");
    }

    #[test]
    fn a_damaged_pack_is_an_error_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut good = Vec::new();
        write(&mut good, ARTWORK, vec![(10, vec![7; 100])]).unwrap();
        for damaged in [
            b"not a pack at all".to_vec(),
            good[..6].to_vec(),
            good[..20].to_vec(),
            good[..good.len() - 1].to_vec(),
        ] {
            std::fs::write(&path, damaged).unwrap();
            assert!(find(&path, ARTWORK, 10).is_err());
        }
    }
}
