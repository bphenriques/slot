//! The library: artists, albums and songs, built from a record of each song's file and tags.
//! Where a tag is missing, the folders stand in, field by field, as laid out in
//! `Music/<Artist>/<Album>/<song>`; a library read from folders alone is the same build with
//! empty tags.

pub mod covers;
pub mod playlists;
pub mod scan;
pub mod tags;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use slot_store::sort_key;

use crate::library::playlists::Playlist;

use crate::library::tags::Tags;
use crate::text;

pub const MUSIC_DIR: &str = "Music";
/// Audiobooks, apart from the music: a folder, or a file, a book.
pub const BOOKS_DIR: &str = "Audiobooks";
/// How many albums Recently Added lists.
const RECENT_ALBUMS: usize = 25;

/// What a track, album, artist or book is known by: its place in the library's list of them,
/// which a rescan renumbers. Each kind is its own type, so one is never taken for another, and
/// indexes only its own list.
macro_rules! ids {
    ($($id:ident => $item:ty),*) => {$(
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
        pub struct $id(pub usize);

        impl std::ops::Index<$id> for [$item] {
            type Output = $item;
            fn index(&self, id: $id) -> &$item {
                &self[id.0]
            }
        }

        impl std::ops::Index<$id> for Vec<$item> {
            type Output = $item;
            fn index(&self, id: $id) -> &$item {
                &self[id.0]
            }
        }

        impl std::ops::IndexMut<$id> for Vec<$item> {
            fn index_mut(&mut self, id: $id) -> &mut $item {
                &mut self[id.0]
            }
        }
    )*};
}

ids!(TrackId => Track, AlbumId => Album, ArtistId => Artist, BookId => Book);

const AUDIO: [&str; 9] = [
    "mp3", "m4a", "m4b", "aac", "flac", "ogg", "opus", "wav", "wave",
];

pub struct Artist {
    pub name: String,
    pub albums: Vec<AlbumId>,
}

pub struct Album {
    pub name: String,
    pub artist: ArtistId,
    pub tracks: Vec<TrackId>,
    /// When its newest song came into the library, and when a song's file last changed, in
    /// seconds.
    pub added: u64,
    pub changed: u64,
    /// When it came out, from its songs' tags; 0 when they do not say.
    pub year: u32,
}

pub struct Track {
    pub title: String,
    /// The song's own artist, which on a compilation is not the album's.
    pub artist: String,
    /// Its place on the album, "03", or "2-03" on an album of several discs; or empty.
    pub number: String,
    /// Which disc of its album it is on, from 1.
    pub disc: u32,
    pub path: PathBuf,
    pub album: AlbumId,
    /// The book it is a part of, and how long it plays, in seconds: 0 unless a book's.
    pub book: Option<BookId>,
    pub secs: f64,
}

/// An audiobook: its title, author, parts and picture are kept as an album's are.
pub struct Book {
    pub album: AlbumId,
    /// What its place is kept by: its folder or file at the top of Audiobooks.
    pub key: PathBuf,
    /// How long it plays, in seconds; 0 when a part's length is not known.
    pub secs: f64,
}

/// A song as the scan found it: its file, when it changed and how big it is, when the library
/// first had it, and its tags.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub path: PathBuf,
    pub modified: u64,
    pub size: u64,
    pub added: u64,
    pub tags: Tags,
}

/// Artists by album artist, their albums, and the songs on them; and the playlists.
#[derive(Default)]
pub struct Library {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub albums_by_name: Vec<AlbumId>,
    pub tracks_by_title: Vec<TrackId>,
    /// The albums last added, newest first; those added together, by their files' dates.
    pub albums_by_added: Vec<AlbumId>,
    /// Favorites first.
    pub playlists: Vec<Playlist>,
    /// The music's artists, as Artists lists them; the books' authors are not among them.
    pub music_artists: Vec<ArtistId>,
    /// The audiobooks, by title.
    pub books: Vec<Book>,
}

/// A song's fields once its tags and folders are put together.
struct Song {
    title: String,
    artist: String,
    album_artist: String,
    album: String,
    disc: u32,
    track: u32,
    year: u32,
    path: PathBuf,
    modified: u64,
    added: u64,
    secs: u32,
}

impl Song {
    fn of(music: &Path, r: Record) -> Song {
        let rel = r.path.strip_prefix(music).unwrap_or(&r.path);
        let dirs: Vec<&str> = rel
            .parent()
            .into_iter()
            .flat_map(Path::iter)
            .filter_map(|d| d.to_str())
            .collect();
        let (folder_artist, folder_album) = match dirs.as_slice() {
            [.., artist, album] => (*artist, *album),
            [artist] => (*artist, ""),
            [] => ("", ""),
        };
        let file = rel.file_name().and_then(|f| f.to_str()).unwrap_or_default();
        // "1-03" from the file name: disc 1, track 3.
        let numbered = track_number(file);
        let mut from_file = numbered.rsplit('-').map(|n| n.parse::<u32>().unwrap_or(0));
        let (file_track, file_disc) = (from_file.next().unwrap_or(0), from_file.next());
        let or = |tag: &str, fallback: &str| match tag.is_empty() {
            true => fallback.to_string(),
            false => tag.to_string(),
        };
        let t = &r.tags;
        let folder_artist = or(folder_artist, text::UNKNOWN_ARTIST);
        let artist = or(&t.artist, &or(&t.album_artist, &folder_artist));
        Song {
            title: or(&t.title, &track_title(file)),
            album_artist: or(&t.album_artist, &artist),
            artist,
            album: or(&t.album, or(folder_album, text::UNKNOWN_ALBUM).as_str()),
            disc: match t.disc {
                0 => file_disc.unwrap_or(1).max(1),
                d => d,
            },
            track: match t.track {
                0 => file_track,
                n => n,
            },
            modified: r.modified,
            added: r.added,
            year: r.tags.year,
            secs: r.tags.secs,
            path: r.path,
        }
    }
}

impl Library {
    /// Each track by its path, to read a file's paths back by.
    pub fn index(&self) -> std::collections::HashMap<&std::path::Path, TrackId> {
        let paths = self.tracks.iter().map(|t| t.path.as_path());
        paths.enumerate().map(|(i, p)| (p, TrackId(i))).collect()
    }

    /// Reads the card afresh, every song's tags and none from the cache.
    pub fn scan(root: &Path) -> Library {
        crate::library::scan::scan(root, true, &crate::library::scan::Progress::default()).0
    }

    /// The library from `records`, in any order, with the playlists in `root`'s music and the
    /// audiobooks after it.
    pub fn build(root: &Path, records: Vec<Record>) -> Library {
        let (music, books) = (root.join(MUSIC_DIR), root.join(BOOKS_DIR));
        let (book_records, records): (Vec<Record>, Vec<Record>) = records
            .into_iter()
            .partition(|r| r.path.starts_with(&books));
        // Album artist, then album, each by its name in any case.
        type Albums = HashMap<String, (String, Vec<Song>)>;
        let mut grouped: HashMap<String, (String, Albums)> = HashMap::new();
        for song in records.into_iter().map(|r| Song::of(&music, r)) {
            let (_, albums) = grouped
                .entry(song.album_artist.to_lowercase())
                .or_insert_with(|| (song.album_artist.clone(), HashMap::new()));
            albums
                .entry(song.album.to_lowercase())
                .or_insert_with(|| (song.album.clone(), Vec::new()))
                .1
                .push(song);
        }
        let mut artists: Vec<(String, Albums)> = grouped.into_values().collect();
        artists.sort_by_cached_key(|(name, _)| sort_key(name));
        let mut lib = Library::default();
        for (name, albums) in artists {
            let artist = ArtistId(lib.artists.len());
            // By year, as players list an artist's albums; those that do not say, after, by name.
            let mut albums: Vec<(String, Vec<Song>)> = albums.into_values().collect();
            albums.sort_by_cached_key(|(name, songs)| {
                let year = year(songs);
                (year == 0, year, sort_key(name))
            });
            let ids = albums
                .into_iter()
                .map(|(name, songs)| lib.add_album(name, artist, songs))
                .collect();
            lib.artists.push(Artist { name, albums: ids });
        }
        lib.albums_by_name = (0..lib.albums.len()).map(AlbumId).collect();
        lib.albums_by_name
            .sort_by_cached_key(|&a| sort_key(&lib.albums[a].name));
        lib.tracks_by_title = (0..lib.tracks.len()).map(TrackId).collect();
        lib.tracks_by_title
            .sort_by_cached_key(|&t| sort_key(&lib.tracks[t].title));
        lib.albums_by_added = (0..lib.albums.len()).map(AlbumId).collect();
        lib.albums_by_added
            .sort_by_key(|&a| std::cmp::Reverse((lib.albums[a].added, lib.albums[a].changed)));
        lib.albums_by_added.truncate(RECENT_ALBUMS);
        lib.music_artists = (0..lib.artists.len()).map(ArtistId).collect();
        lib.add_books(&books, book_records);
        lib.playlists = crate::store::playlists::read(root, &lib);
        lib
    }

    /// Files an album's songs in disc and track order, then by file name.
    fn add_album(&mut self, name: String, artist: ArtistId, mut songs: Vec<Song>) -> AlbumId {
        songs.sort_by_cached_key(|s| {
            (
                s.disc,
                s.track,
                s.path.file_name().map(|f| f.to_ascii_lowercase()),
            )
        });
        let discs = songs.iter().any(|s| s.disc > 1);
        let album = AlbumId(self.albums.len());
        let added = songs.iter().map(|s| s.added).max().unwrap_or(0);
        let changed = songs.iter().map(|s| s.modified).max().unwrap_or(0);
        let year = year(&songs);
        let tracks = songs
            .into_iter()
            .map(|s| {
                self.tracks.push(Track {
                    number: match (s.track, discs) {
                        (0, _) => String::new(),
                        (n, true) => format!("{}-{n:02}", s.disc),
                        (n, false) => format!("{n:02}"),
                    },
                    disc: s.disc,
                    title: s.title,
                    artist: s.artist,
                    path: s.path,
                    album,
                    book: None,
                    secs: f64::from(s.secs),
                });
                TrackId(self.tracks.len() - 1)
            })
            .collect();
        self.albums.push(Album {
            name,
            artist,
            tracks,
            added,
            changed,
            year,
        });
        album
    }
    /// The books under `dir`: each folder at its top is a book, whatever its depth, and so is
    /// each file there; its parts in order. They are kept as albums are, for their names, parts
    /// and pictures, under their authors, but listed only as books.
    fn add_books(&mut self, dir: &Path, records: Vec<Record>) {
        let mut grouped: HashMap<PathBuf, Vec<Record>> = HashMap::new();
        for r in records {
            let rel = r.path.strip_prefix(dir).unwrap_or(&r.path);
            let top = rel.iter().next().map(PathBuf::from).unwrap_or_default();
            grouped.entry(top).or_default().push(r);
        }
        let mut books: Vec<(PathBuf, String, String, Vec<Record>)> = grouped
            .into_iter()
            .map(|(key, records)| {
                // "Author - Title" names a book its tags do not.
                let stem = key.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let (by, named) = stem.split_once(" - ").unwrap_or(("", stem));
                let tag = |pick: fn(&Tags) -> &String| {
                    let mut tags = records.iter().map(|r| pick(&r.tags));
                    tags.find(|t| !t.is_empty()).cloned()
                };
                let title = tag(|t| &t.album).unwrap_or_else(|| named.to_string());
                let author = tag(|t| &t.album_artist)
                    .or_else(|| tag(|t| &t.artist))
                    .or_else(|| (!by.is_empty()).then(|| by.to_string()))
                    .unwrap_or_else(|| text::UNKNOWN_ARTIST.to_string());
                (key, title, author, records)
            })
            .collect();
        books.sort_by_cached_key(|(_, title, ..)| sort_key(title));
        let mut authors: HashMap<String, ArtistId> = HashMap::new();
        for (key, title, author, records) in books {
            let artist = *authors.entry(author.to_lowercase()).or_insert_with(|| {
                self.artists.push(Artist {
                    name: author,
                    albums: Vec::new(),
                });
                ArtistId(self.artists.len() - 1)
            });
            let songs = records.into_iter().map(|r| Song::of(dir, r)).collect();
            let album = self.add_album(title, artist, songs);
            self.artists[artist].albums.push(album);
            let book = BookId(self.books.len());
            for &t in &self.albums[album].tracks {
                self.tracks[t].book = Some(book);
            }
            let secs = self.albums[album]
                .tracks
                .iter()
                .map(|&t| self.tracks[t].secs);
            self.books.push(Book {
                album,
                key,
                secs: secs.sum(),
            });
        }
    }

    /// The book track `t` is a part of, if any.
    pub fn book_of(&self, t: TrackId) -> Option<&Book> {
        self.books.get(self.tracks.get(t.0)?.book?.0)
    }

    /// Every song, artist by artist, as Artists plays them; a book's parts are not songs.
    pub fn music_tracks(&self) -> Vec<TrackId> {
        let artists = self.music_artists.iter();
        artists.flat_map(|&a| self.artist_tracks(a)).collect()
    }

    pub fn artist_tracks(&self, artist: ArtistId) -> Vec<TrackId> {
        self.artists[artist]
            .albums
            .iter()
            .flat_map(|&a| self.albums[a].tracks.iter().copied())
            .collect()
    }

    /// The album's artist, under whom the song is filed.
    pub fn artist_of(&self, track: TrackId) -> &Artist {
        &self.artists[self.albums[self.tracks[track].album].artist]
    }

    pub fn album_of(&self, track: TrackId) -> &Album {
        &self.albums[self.tracks[track].album]
    }
}

/// The year an album came out: its songs' earliest, as a reissue keeps the first's; 0 when none
/// says.
fn year(songs: &[Song]) -> u32 {
    songs
        .iter()
        .map(|s| s.year)
        .filter(|&y| y > 0)
        .min()
        .unwrap_or(0)
}

pub(crate) fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

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
