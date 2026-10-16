//! Finds the songs in the music folder, at any depth, and reads their tags. The tags are kept in
//! a cache on the card with each file's date and size, so a later scan reads only the songs
//! that are new or changed; a full scan reads them all again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use slot_store::is_hidden;

use crate::library::{is_audio, Library, Record, BOOKS_DIR, MUSIC_DIR};
use crate::playlists::PLAYLISTS_DIR;
use crate::tags::{self, Tags};

pub const CACHE_FILE: &str = "Config/music-library.tsv";
/// The cache's first line; a cache in another layout is read as empty.
const HEADER: &str = "#SLOT-LIBRARY 4";
/// Folders this deep are not looked in, so a link to a folder above cannot go round forever.
const MAX_DEPTH: usize = 8;

/// How far a scan has got: files walked, of those the cache knows; then songs read, of those to
/// read.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub of: AtomicUsize,
}

/// The library as the cache last had it, without looking at the card's folders; none without a
/// cache.
pub fn cached(root: &Path) -> Option<Library> {
    let records = read_cache(root);
    (!records.is_empty()).then(|| Library::build(root, records.into_values().collect()))
}

/// The library under `root`, and whether it differs from the cache's: every song read again
/// when `full`, otherwise only those the cache does not have as they are.
pub fn scan(root: &Path, full: bool, progress: &Progress) -> (Library, bool) {
    // A full scan reads every song again, but keeps when each came into the library.
    let mut cached = read_cache(root);
    progress.of.store(cached.len(), Ordering::Relaxed);
    let mut files = Vec::new();
    let books = root.join(BOOKS_DIR);
    walk(&root.join(MUSIC_DIR), 0, &mut files, progress);
    walk(&books, 0, &mut files, progress);
    let fresh = |path: &Path, modified: u64, size: u64| {
        !full
            && cached
                .get(path)
                .is_some_and(|r| (r.modified, r.size) == (modified, size))
    };
    let to_read = files.iter().filter(|(p, m, s)| !fresh(p, *m, *s)).count();
    progress.done.store(0, Ordering::Relaxed);
    progress.of.store(to_read, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut read = 0;
    let records: Vec<Record> = files
        .into_iter()
        .map(|(path, modified, size)| {
            let known = cached.remove(&path);
            match known {
                Some(r) if !full && (r.modified, r.size) == (modified, size) => r,
                _ => {
                    read += 1;
                    progress.done.fetch_add(1, Ordering::Relaxed);
                    Record {
                        // A book's parts are timed, for how far into it the listener is.
                        tags: tags::read(&path, path.starts_with(&books)),
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
    eprintln!("slot: music: {} songs, {read} read", records.len());
    (Library::build(root, records), changed)
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
        let is_dir = match kind.is_symlink() {
            true => path.is_dir(),
            false => kind.is_dir(),
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
            progress.done.fetch_add(1, Ordering::Relaxed);
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
        eprintln!("slot: music: {}: {e}", path.display());
    }
}

fn read_cache(root: &Path) -> HashMap<PathBuf, Record> {
    let text = std::fs::read_to_string(root.join(CACHE_FILE)).unwrap_or_default();
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return HashMap::new();
    }
    lines
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [path, modified, size, added, title, artist, album_artist, album, disc, track, year, secs] =
                f[..]
            else {
                return None;
            };
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
                },
            };
            Some((path, record))
        })
        .collect()
}
