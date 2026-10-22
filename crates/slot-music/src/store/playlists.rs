//! Playlist files: plain `.m3u` and `.m3u8` in `Music/Playlists`, as other players read and
//! write them, each song's path from the playlist's folder.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::library::sort::sort_key;
use crate::store::file::is_hidden;

use super::Store;
use crate::library::playlists::Playlist;
use crate::library::{Library, TrackId};

/// Where playlists live, in the music folder; the library does not read it as an artist.
pub const PLAYLISTS_DIR: &str = "Playlists";
const FAVORITES: &str = "Favorites";

impl Store {
    #[cfg(test)]
    /// Writes `list` back, whole or not at all: a power-off mid-write never leaves Favorites
    /// cut short.
    pub fn write_playlist(&self, list: &Playlist, lib: &Library) {
        self.write(&list.path, encode(list, lib));
    }
}

/// The playlists, Favorites first whether or not it has been written yet, then the rest by name.
/// Lines that name no track in the library are left out. Read with the library, from the
/// music's folder, `music`.
pub fn read(music: &Path, lib: &Library) -> Vec<Playlist> {
    let dir = music.join(PLAYLISTS_DIR);
    let index = lib.index();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| !is_hidden(p) && is_playlist(p))
        .collect();
    files.sort_by_cached_key(|p| sort_key(&name(p)));
    let favorites = files
        .iter()
        .position(|p| name(p).eq_ignore_ascii_case(FAVORITES))
        .map(|i| files.remove(i))
        .unwrap_or_else(|| dir.join(format!("{FAVORITES}.m3u8")));
    std::iter::once(favorites)
        .chain(files)
        .map(|path| Playlist {
            name: name(&path),
            tracks: tracks(&path, music, &index),
            path,
        })
        .collect()
}

/// `list` as its file has it, each song's path from the playlist's folder, as players expect.
pub fn encode(list: &Playlist, lib: &Library) -> String {
    let dir = list.path.parent().unwrap_or(Path::new("."));
    let mut text = String::from("#EXTM3U\n");
    for &t in &list.tracks {
        let path = &lib.tracks[t].path;
        let rel = path
            .strip_prefix(dir.parent().unwrap_or(dir))
            .map_or(path.to_path_buf(), |rel| Path::new("..").join(rel));
        text.push_str(&rel.to_string_lossy());
        text.push('\n');
    }
    text
}

/// The songs a playlist names, by path: from its own folder, from the music's or the folder it is
/// in (the card), or whole.
fn tracks(path: &Path, music: &Path, index: &HashMap<&Path, TrackId>) -> Vec<TrackId> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let text = text(bytes);
    let dir = path.parent().unwrap_or(music);
    let card = music.parent().unwrap_or(music);
    text.lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            // Windows players write their folders with backslashes.
            let line = line.replace('\\', "/");
            let line = Path::new(&line);
            [dir, card, music]
                .into_iter()
                .find_map(|base| index.get(normal(&base.join(line)).as_path()).copied())
        })
        .collect()
}

/// A playlist's text: UTF-8 as `.m3u8` promises, or else Latin-1, as older `.m3u` files are.
fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|e| e.into_bytes().into_iter().map(char::from).collect())
}

/// `path` with its `.` and `..` worked out, without asking the card.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            part => out.push(part),
        }
    }
    out
}

fn is_playlist(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
}

fn name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
