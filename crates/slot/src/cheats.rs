//! The cheats of the game in the slot: its own `.cht` file in `Cheats/<GBA|GB|GBC>/`, named like
//! the ROM, or else its entry in `Cheats/Cheats.pack`. Which are on is remembered per game in
//! `Config/Cheats/<GBA|GB|GBC>/<rom>.txt`, one name a line.

use std::path::{Path, PathBuf};

use slot_store::Platform;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cheat {
    pub name: String,
    pub code: String,
    pub on: bool,
}

fn own_file(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    root.join("Cheats")
        .join(platform.dir_name())
        .join(format!("{stem}.cht"))
}

fn state_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    root.join(slot_store::CONFIG_DIR)
        .join("Cheats")
        .join(platform.dir_name())
        .join(format!("{stem}.txt"))
}

/// A CodeBreaker master code and nothing else: the game's ID and a hook, the place in the game's
/// code where the cheats are to run. mGBA runs cheats every frame without one, but with one only
/// when the game reaches the hook, which a revision the code was not written for never does.
fn hook_only(code: &str) -> bool {
    let words: Vec<&str> = code.split(['+', ' ']).filter(|w| !w.is_empty()).collect();
    !words.is_empty()
        && words.chunks(2).all(|pair| {
            matches!(pair, [address, value]
                if address.len() == 8 && value.len() == 4
                    && (address.starts_with('0') || address.starts_with('1'))
                    && [address, value].iter().all(|w| w.bytes().all(|b| b.is_ascii_hexdigit())))
        })
}

/// The game's cheats, with the ones it left on. A file of its own wins over the pack.
pub fn load(root: &Path, platform: Platform, stem: &str, rom: &Path) -> Vec<Cheat> {
    let found = match std::fs::read(own_file(root, platform, stem)) {
        Ok(bytes) => slot_labels::cheats::parse(&String::from_utf8_lossy(&bytes)),
        Err(_) => slot_labels::cheats::from_pack(root, rom),
    };
    let saved = std::fs::read_to_string(state_path(root, platform, stem)).unwrap_or_default();
    let on: Vec<&str> = saved.lines().map(str::trim).collect();
    let mut cheats: Vec<Cheat> = Vec::new();
    for c in found {
        if hook_only(&c.code) || cheats.iter().any(|k| k.name == c.name) {
            continue;
        }
        cheats.push(Cheat {
            on: on.contains(&c.name.as_str()),
            name: c.name,
            code: c.code,
        });
    }
    cheats
}

/// Remembers which cheats are on; with none on, the game's file goes.
pub fn save(root: &Path, platform: Platform, stem: &str, cheats: &[Cheat]) {
    let path = state_path(root, platform, stem);
    let on: Vec<&str> = cheats
        .iter()
        .filter(|c| c.on)
        .map(|c| c.name.as_str())
        .collect();
    let written = match on.is_empty() {
        true => match std::fs::remove_file(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            done => done,
        },
        false => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| slot_store::atomic_write(&path, (on.join("\n") + "\n").as_bytes())),
    };
    if let Err(e) = written {
        eprintln!("slot: cheats: {}: {e}", path.display());
    }
}

/// The codes of the cheats that are on, for the core.
pub fn codes(cheats: &[Cheat]) -> Vec<String> {
    cheats
        .iter()
        .filter(|c| c.on)
        .map(|c| c.code.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Games/GBA/Fusion.gba");
        std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
        std::fs::write(&rom, b"rom").unwrap();
        std::fs::create_dir_all(dir.path().join("Cheats/GBA")).unwrap();
        (dir, rom)
    }

    const CHT: &str = "cheat0_desc = \"Enable Code (Must Be On)\"\ncheat0_code = \"00000F6D+000A+100008B4+0007\"\n\
                       cheat1_desc = \"Infinite Health\"\ncheat1_code = \"83001310+0063\"\n\
                       cheat2_desc = \"Max Missiles\"\ncheat2_code = \"83001312+00FF\"\n";

    #[test]
    fn a_games_own_file_is_read_without_its_hook_and_nothing_is_on_at_first() {
        let (dir, rom) = card();
        std::fs::write(dir.path().join("Cheats/GBA/Fusion.cht"), CHT).unwrap();
        let cheats = load(dir.path(), Platform::Gba, "Fusion", &rom);
        let names: Vec<&str> = cheats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Infinite Health", "Max Missiles"]);
        assert!(cheats.iter().all(|c| !c.on));
        assert!(codes(&cheats).is_empty());
    }

    #[test]
    fn the_cheats_left_on_come_back_on_and_none_on_leaves_no_file() {
        let (dir, rom) = card();
        std::fs::write(dir.path().join("Cheats/GBA/Fusion.cht"), CHT).unwrap();
        let mut cheats = load(dir.path(), Platform::Gba, "Fusion", &rom);
        cheats[1].on = true;
        save(dir.path(), Platform::Gba, "Fusion", &cheats);
        let again = load(dir.path(), Platform::Gba, "Fusion", &rom);
        assert_eq!(codes(&again), ["83001312+00FF"]);
        let other = load(dir.path(), Platform::Gba, "Zero Mission", &rom);
        assert!(codes(&other).is_empty());
        cheats[1].on = false;
        save(dir.path(), Platform::Gba, "Fusion", &cheats);
        assert!(!state_path(dir.path(), Platform::Gba, "Fusion").exists());
        save(dir.path(), Platform::Gba, "Fusion", &cheats);
    }

    #[test]
    fn no_file_and_no_pack_is_no_cheats() {
        let (dir, rom) = card();
        assert!(load(dir.path(), Platform::Gba, "Fusion", &rom).is_empty());
    }

    #[test]
    fn only_a_bare_codebreaker_hook_is_left_out() {
        assert!(hook_only("000039C7+000A+10068310+0007"));
        assert!(hook_only("00004D4E 000A 1014FA3C 0007"));
        assert!(!hook_only("00003067+000A+1000036C+0007+82001790+0000"));
        assert!(!hook_only("32000410+0005"));
        assert!(!hook_only("D8BAE4D9+4864DCE5+A86CDBA5+19BA49B3"));
        assert!(!hook_only("010903C2"));
        assert!(!hook_only("00A-17B-C49"));
    }
}
