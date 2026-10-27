//! Cheats as RetroArch keeps them in `.cht` files, and the cheat pack: the libretro database's
//! cheats for every dump they could be matched to, by the dump's CRC32, in `Cheats/Cheats.pack`.
use std::collections::BTreeMap;
use std::path::Path;

use crate::pack;

pub const PACK: &str = "Cheats.pack";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cheat {
    pub name: String,
    /// One or more codes joined by `+`, as the core reads them.
    pub code: String,
}

fn unquote(v: &str) -> &str {
    let v = v.trim();
    let v = v.strip_prefix('"').unwrap_or(v);
    v.strip_suffix('"').unwrap_or(v)
}

/// The cheats in a `.cht` file, in its order. A cheat without a code is left out: RetroArch's
/// own memory-search cheats have an address and value instead, and only RetroArch can run them.
/// A cheat without a name is named by its code.
pub fn parse(text: &str) -> Vec<Cheat> {
    let mut slots: BTreeMap<usize, (Option<&str>, Option<&str>)> = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some((index, field)) = key
            .trim()
            .strip_prefix("cheat")
            .and_then(|r| r.split_once('_'))
        else {
            continue;
        };
        let Ok(i) = index.parse::<usize>() else {
            continue;
        };
        let slot = slots.entry(i).or_default();
        match field {
            "desc" => slot.0 = Some(unquote(value)),
            "code" => slot.1 = Some(unquote(value)),
            _ => {}
        }
    }
    slots
        .into_values()
        .filter_map(|(name, code)| {
            let code = code?.trim();
            if code.is_empty() {
                return None;
            }
            let name = name
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(code);
            Some(Cheat {
                name: name.to_owned(),
                code: code.to_owned(),
            })
        })
        .collect()
}

/// The cheats as a `.cht` file, all off.
pub fn write(cheats: &[Cheat]) -> String {
    let mut out = format!("cheats = {}\n", cheats.len());
    for (i, c) in cheats.iter().enumerate() {
        out.push_str(&format!(
            "\ncheat{i}_desc = \"{}\"\ncheat{i}_code = \"{}\"\ncheat{i}_enable = false\n",
            c.name.replace('"', "'"),
            c.code
        ));
    }
    out
}

/// This ROM's cheats from the card's cheat pack. No pack, or nothing in it for this dump, is
/// none; a damaged pack is reported and passed over. The ROM is only read when there is a pack.
pub fn from_pack(root: &Path, rom: &Path) -> Vec<Cheat> {
    let path = root.join("Cheats").join(PACK);
    if !path.is_file() {
        return Vec::new();
    }
    let found = crate::catalog::crc32(rom).and_then(|crc| pack::find(&path, pack::CHEATS, crc));
    match found {
        Ok(Some(text)) => parse(&String::from_utf8_lossy(&text)),
        Ok(None) => Vec::new(),
        Err(e) => {
            eprintln!("slot: cheats: {}: {e}", path.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cheat(name: &str, code: &str) -> Cheat {
        Cheat {
            name: name.into(),
            code: code.into(),
        }
    }

    #[test]
    fn reads_names_and_codes_and_skips_what_the_core_cannot_run() {
        let text = "cheats = 4\r\n\r\ncheat0_desc = \"Master Code\"\r\ncheat0_code = \"AAAA+BBBB\"\r\ncheat0_enable = false\r\n\
                    cheat1_desc = \"Nothing\"\ncheat1_code = \"\"\n\
                    cheat2_desc = \"Search result\"\ncheat2_address = \"123\"\ncheat2_value = \"9\"\n\
                    cheat3_code = \"1234\"\n";
        assert_eq!(
            parse(text),
            vec![cheat("Master Code", "AAAA+BBBB"), cheat("1234", "1234")]
        );
    }

    #[test]
    fn cheats_come_in_index_order_whatever_the_file_order() {
        let text = "cheat10_desc = \"Ten\"\ncheat10_code = \"10\"\ncheat2_desc = \"Two\"\ncheat2_code = \"2\"\n";
        assert_eq!(parse(text), vec![cheat("Two", "2"), cheat("Ten", "10")]);
    }

    #[test]
    fn a_written_file_reads_back_the_same() {
        let cheats = vec![
            cheat("Infinite Health", "32000410+0005"),
            cheat("Max \"Money\"", "1"),
        ];
        let back = parse(&write(&cheats));
        assert_eq!(back[0], cheats[0]);
        assert_eq!(back[1], cheat("Max 'Money'", "1"));
    }

    #[test]
    fn a_rom_finds_its_cheats_in_the_pack_by_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Game.gba");
        std::fs::write(&rom, b"a rom").unwrap();
        assert!(from_pack(dir.path(), &rom).is_empty());
        let crc = crate::catalog::crc32(&rom).unwrap();
        let text = write(&[cheat("Infinite Lives", "32000B08+0009")]);
        let mut out = Vec::new();
        pack::write(&mut out, pack::CHEATS, vec![(crc, text.into_bytes())]).unwrap();
        std::fs::create_dir_all(dir.path().join("Cheats")).unwrap();
        std::fs::write(dir.path().join("Cheats").join(PACK), out).unwrap();
        assert_eq!(
            from_pack(dir.path(), &rom),
            vec![cheat("Infinite Lives", "32000B08+0009")]
        );
        std::fs::write(&rom, b"another rom").unwrap();
        assert!(from_pack(dir.path(), &rom).is_empty());
    }

    #[test]
    fn an_artwork_pack_in_the_cheats_folder_is_passed_over() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Game.gba");
        std::fs::write(&rom, b"a rom").unwrap();
        let crc = crate::catalog::crc32(&rom).unwrap();
        let mut out = Vec::new();
        pack::write(&mut out, pack::ARTWORK, vec![(crc, b"jpeg".to_vec())]).unwrap();
        std::fs::create_dir_all(dir.path().join("Cheats")).unwrap();
        std::fs::write(dir.path().join("Cheats").join(PACK), out).unwrap();
        assert!(from_pack(dir.path(), &rom).is_empty());
    }
}
