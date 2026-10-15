use std::path::{Path, PathBuf};

use slot_store::{is_hidden, sort_key};

pub const MUSIC_DIR: &str = "Music";

const AUDIO: [&str; 9] = [
    "mp3", "m4a", "m4b", "aac", "flac", "ogg", "opus", "wav", "wave",
];

pub struct Artist {
    pub name: String,
    pub albums: Vec<usize>,
}

pub struct Album {
    pub name: String,
    pub artist: usize,
    pub tracks: Vec<usize>,
}

pub struct Track {
    pub title: String,
    /// The number the file name starts with, "03" or "1-03", or empty.
    pub number: String,
    pub path: PathBuf,
    pub album: usize,
}

/// `Music/<Artist>/<Album>/<track>`, from folder and file names only.
#[derive(Default)]
pub struct Library {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub albums_by_name: Vec<usize>,
    pub tracks_by_title: Vec<usize>,
}

impl Library {
    pub fn scan(root: &Path) -> Library {
        let mut lib = Library::default();
        for (name, artist_dir) in sorted(&root.join(MUSIC_DIR), true) {
            let artist = lib.artists.len();
            let mut albums = Vec::new();
            for (name, album_dir) in sorted(&artist_dir, true) {
                let album = lib.albums.len();
                let tracks: Vec<usize> = sorted(&album_dir, false)
                    .into_iter()
                    .filter(|(_, path)| is_audio(path))
                    .map(|(name, path)| {
                        lib.tracks.push(Track {
                            title: track_title(&name),
                            number: track_number(&name),
                            path,
                            album,
                        });
                        lib.tracks.len() - 1
                    })
                    .collect();
                if tracks.is_empty() {
                    continue;
                }
                lib.albums.push(Album {
                    name,
                    artist,
                    tracks,
                });
                albums.push(album);
            }
            if !albums.is_empty() {
                lib.artists.push(Artist { name, albums });
            }
        }
        lib.albums_by_name = (0..lib.albums.len()).collect();
        lib.albums_by_name
            .sort_by_cached_key(|&a| sort_key(&lib.albums[a].name));
        lib.tracks_by_title = (0..lib.tracks.len()).collect();
        lib.tracks_by_title
            .sort_by_cached_key(|&t| sort_key(&lib.tracks[t].title));
        lib
    }

    pub fn artist_tracks(&self, artist: usize) -> Vec<usize> {
        self.artists[artist]
            .albums
            .iter()
            .flat_map(|&a| self.albums[a].tracks.iter().copied())
            .collect()
    }

    pub fn artist_of(&self, track: usize) -> &Artist {
        &self.artists[self.albums[self.tracks[track].album].artist]
    }

    pub fn album_of(&self, track: usize) -> &Album {
        &self.albums[self.tracks[track].album]
    }
}

/// The folders (`dirs`) or files in `dir`: folders sorted like the shelf, files by name.
fn sorted(dir: &Path, dirs: bool) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    // The listing gives each entry's type, saving a read per entry.
    let mut out: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| match e.file_type() {
            Ok(t) if t.is_symlink() => e.path().is_dir() == dirs,
            Ok(t) => t.is_dir() == dirs,
            Err(_) => false,
        })
        .map(|e| e.path())
        .filter(|p| !is_hidden(p))
        .filter_map(|p| Some((p.file_name()?.to_str()?.to_string(), p)))
        .collect();
    match dirs {
        true => out.sort_by_cached_key(|(name, _)| sort_key(name)),
        false => out.sort_by_cached_key(|(name, _)| name.to_lowercase()),
    }
    out
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// "03 Dream On.mp3" is number "03"; a name that is all number, like "1999.mp3", has none.
pub fn track_number(file_name: &str) -> String {
    let title = track_title(file_name);
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    match stem.strip_suffix(title.as_str()) {
        Some(head) if !head.is_empty() => head.trim_end_matches([' ', '.', '-', '_']).to_string(),
        _ => String::new(),
    }
}

/// "03 Dream On.mp3" and "1-03 - Dream On.flac" read as "Dream On".
pub fn track_title(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let rest = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-');
    let numbered = rest.len() < stem.len();
    let title = rest.trim_start_matches([' ', '.', '-', '_']);
    match numbered && !title.is_empty() && title.len() < rest.len() {
        true => title.to_string(),
        false => stem.to_string(),
    }
}
