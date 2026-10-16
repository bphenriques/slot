//! Playlists: plain `.m3u` and `.m3u8` files in `Music/Playlists`, as other players read and
//! write them. Favorites is one of them, always listed, which slot adds songs to and takes them
//! from.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use slot_store::{is_hidden, sort_key};

use crate::library::{Library, MUSIC_DIR};

/// Where playlists live, in the music folder; the library does not read it as an artist.
pub const PLAYLISTS_DIR: &str = "Playlists";
const FAVORITES: &str = "Favorites";

pub struct Playlist {
    pub name: String,
    pub path: PathBuf,
    pub tracks: Vec<usize>,
}

/// The playlists, Favorites first whether or not it has been written yet, then the rest by name.
/// Lines that name no track in the library are left out.
pub fn read(root: &Path, lib: &Library) -> Vec<Playlist> {
    let dir = root.join(MUSIC_DIR).join(PLAYLISTS_DIR);
    let index: HashMap<&Path, usize> = lib
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.path.as_path(), i))
        .collect();
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
            tracks: tracks(&path, root, &index),
            path,
        })
        .collect()
}

/// Favorites, the first playlist: songs only, kept as any other playlist file.
impl Library {
    pub fn is_favorite(&self, t: usize) -> bool {
        self.playlists
            .first()
            .is_some_and(|f| f.tracks.contains(&t))
    }

    /// Puts song `t` in Favorites, or takes it out, and writes the file. A queue dealt from
    /// Favorites before keeps its songs.
    pub fn toggle_favorite(&mut self, t: usize) {
        let Some(favorites) = self.playlists.first_mut() else {
            return;
        };
        match favorites.tracks.contains(&t) {
            true => favorites.tracks.retain(|&f| f != t),
            false => favorites.tracks.push(t),
        }
        write(&self.playlists[0], self);
    }
}

/// Writes `list` back, with each song's path from the playlist's folder, as players expect.
pub fn write(list: &Playlist, lib: &Library) {
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
    let done = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&list.path, text));
    if let Err(e) = done {
        eprintln!("slot: music: {}: {e}", list.path.display());
    }
}

/// The songs a playlist names, by path: from its own folder, from the card, or whole.
fn tracks(path: &Path, root: &Path, index: &HashMap<&Path, usize>) -> Vec<usize> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let text = decode(bytes);
    let dir = path.parent().unwrap_or(root);
    text.lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            // Windows players write their folders with backslashes.
            let line = line.replace('\\', "/");
            let line = Path::new(&line);
            [dir, root, root.join(MUSIC_DIR).as_path()]
                .into_iter()
                .find_map(|base| index.get(normal(&base.join(line)).as_path()).copied())
        })
        .collect()
}

/// A playlist's text: UTF-8 as `.m3u8` promises, or else Latin-1, as older `.m3u` files are.
fn decode(bytes: Vec<u8>) -> String {
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
