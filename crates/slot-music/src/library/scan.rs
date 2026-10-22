//! Finds the songs in the music folder, at any depth, and reads their tags. The tags are kept in
//! a cache on the card with each file's date and size, so a later scan reads only the songs
//! that are new or changed; a full scan reads them all again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use crate::store::file::is_hidden;

use crate::library::tags::{self, Tags};
use crate::library::{is_audio, Dirs, Library, Record};
use crate::store::playlists::PLAYLISTS_DIR;

pub const CACHE_FILE: &str = "Config/music-library.tsv";
/// The cache's first line; a cache in another layout is read as empty, but for those before,
/// read for when each song came in and to show at once, its songs' tags read again: for genre
/// (4), and for how long songs play, once read only for books (5).
const HEADER: &str = "#SLOT-LIBRARY 6";
const BEFORE: [&str; 2] = ["#SLOT-LIBRARY 4", "#SLOT-LIBRARY 5"];
/// Folders this deep are not looked in, so a link to a folder above cannot go round forever.
const MAX_DEPTH: usize = 8;

/// How far a scan has got: files walked, of those the cache knows; then songs read, of those to
/// read. The two are one value, so a reader never sees one from before the other.
#[derive(Default)]
pub struct Progress(Mutex<(usize, usize)>);

impl Progress {
    /// So far, and of how many.
    pub fn get(&self) -> (usize, usize) {
        *self.lock()
    }

    /// None done yet, of `of`.
    fn start(&self, of: usize) {
        *self.lock() = (0, of);
    }

    fn step(&self) {
        self.lock().0 += 1;
    }

    /// So far, and of how many, as a test has it.
    #[cfg(test)]
    pub fn set(&self, done: usize, of: usize) {
        *self.lock() = (done, of);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (usize, usize)> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The library as the cache last had it, without looking at the card's folders; none without a
/// cache.
pub fn cached(root: &Path, dirs: &Dirs) -> Option<Library> {
    let (records, _) = read_cache(root);
    (!records.is_empty()).then(|| Library::build(dirs, records.into_values().collect()))
}

/// The library in `dirs`, its cache kept on the card at `root`, and whether it differs from the
/// cache's: every song read again when `full`, otherwise only those the cache does not have as
/// they are.
pub fn scan(root: &Path, dirs: &Dirs, full: bool, progress: &Progress) -> (Library, bool) {
    // A full scan reads every song again, but keeps when each came into the library.
    let (mut cached, stale) = read_cache(root);
    progress.start(cached.len());
    let mut files = Vec::new();
    let books = &dirs.books;
    walk(&dirs.music, 0, &mut files, progress);
    walk(books, 0, &mut files, progress);
    let fresh = |path: &Path, modified: u64, size: u64| {
        !full
            && !stale
            && cached
                .get(path)
                .is_some_and(|r| (r.modified, r.size) == (modified, size))
    };
    let to_read = files.iter().filter(|(p, m, s)| !fresh(p, *m, *s)).count();
    progress.start(to_read);
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut read = 0;
    let records: Vec<Record> = files
        .into_iter()
        .map(|(path, modified, size)| {
            let known = cached.remove(&path);
            match known {
                Some(r) if !full && !stale && (r.modified, r.size) == (modified, size) => r,
                _ => {
                    read += 1;
                    progress.step();
                    Record {
                        tags: guarded(&path, || tags::read(&path)),
                        added: known.map_or(now, |r| r.added),
                        path,
                        modified,
                        size,
                    }
                }
            }
        })
        .collect();
    // Songs read, or gone since: the cache is out of date.
    let changed = read > 0 || !cached.is_empty();
    if changed {
        write_cache(root, &records);
    }
    log::info!("{} songs, {read} read", records.len());
    (Library::build(dirs, records), changed)
}

/// `read` of the song at `path`, or nothing known of it if reading it panics, as a malformed tag
/// may: one bad file is a song untagged, not a scan that never ends.
fn guarded<T: Default>(path: &Path, read: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).unwrap_or_else(|_| {
        log::warn!("{}: its tags could not be read", path.display());
        T::default()
    })
}

/// The songs in `dir` and below, each with when it changed and its size, counted into
/// `progress` as found. The playlists' folder at the top is not music.
fn walk(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64, u64)>, progress: &Progress) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_hidden(&path) || (depth == 0 && entry.file_name() == PLAYLISTS_DIR) {
            continue;
        }
        // The listing says what each entry is; only songs, and links, are asked about further.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let is_dir = if kind.is_symlink() {
            path.is_dir()
        } else {
            kind.is_dir()
        };
        if is_dir {
            if depth < MAX_DEPTH {
                walk(&path, depth + 1, out, progress);
            }
        } else if is_audio(&path) {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            out.push((path, modified.map_or(0, |d| d.as_secs()), meta.len()));
            progress.step();
        }
    }
}

/// One line a song: its path from the card, date, size, when it came in, then its tags.
fn write_cache(root: &Path, records: &[Record]) {
    let mut text = format!("{HEADER}\n");
    for r in records {
        let path = r
            .path
            .strip_prefix(root)
            .unwrap_or(&r.path)
            .to_string_lossy();
        if path.contains(['\t', '\n']) {
            continue;
        }
        let t = &r.tags;
        let fields = [
            path.as_ref(),
            &r.modified.to_string(),
            &r.size.to_string(),
            &r.added.to_string(),
            &t.title,
            &t.artist,
            &t.album_artist,
            &t.album,
            &t.disc.to_string(),
            &t.track.to_string(),
            &t.year.to_string(),
            &t.secs.to_string(),
            &t.genre,
        ];
        text.push_str(&fields.join("\t"));
        text.push('\n');
    }
    let path = root.join(CACHE_FILE);
    let part = path.with_extension("tsv.part");
    let done = std::fs::create_dir_all(path.parent().unwrap_or(root))
        .and_then(|_| std::fs::write(&part, text))
        .and_then(|_| std::fs::rename(&part, &path));
    if let Err(e) = done {
        log::warn!("{}: {e}", path.display());
    }
}

/// The songs the cache has, and whether it is of a layout before, its tags to read again.
fn read_cache(root: &Path) -> (HashMap<PathBuf, Record>, bool) {
    let text = std::fs::read_to_string(root.join(CACHE_FILE)).unwrap_or_default();
    let mut lines = text.lines();
    let stale = match lines.next() {
        Some(HEADER) => false,
        Some(line) if BEFORE.contains(&line) => true,
        _ => return (HashMap::new(), false),
    };
    let records = lines
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            // The layout before genres ends at its length.
            let [path, modified, size, added, title, artist, album_artist, album, disc, track, year, secs, ref genre @ ..] =
                f[..]
            else {
                return None;
            };
            let genre = genre.first().copied().unwrap_or_default();
            let path = root.join(path);
            let record = Record {
                path: path.clone(),
                modified: modified.parse().ok()?,
                size: size.parse().ok()?,
                added: added.parse().ok()?,
                tags: Tags {
                    title: title.into(),
                    artist: artist.into(),
                    album_artist: album_artist.into(),
                    album: album.into(),
                    disc: disc.parse().ok()?,
                    track: track.parse().ok()?,
                    year: year.parse().ok()?,
                    secs: secs.parse().ok()?,
                    genre: genre.into(),
                },
            };
            Some((path, record))
        })
        .collect();
    (records, stale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_whose_tags_panic_as_they_are_read_is_a_song_untagged() {
        let tags = guarded(Path::new("bad.mp3"), || -> Tags {
            panic!("a malformed frame")
        });
        assert_eq!(tags, Tags::default());
        let read = guarded(Path::new("good.mp3"), || Tags {
            title: "Song".into(),
            ..Tags::default()
        });
        assert_eq!(read.title, "Song");
    }
}
