//! The library: artists, albums and songs, built from a record of each song's file and tags.
//! Where a tag is missing, the folders stand in, field by field, as laid out in
//! `Music/<Artist>/<Album>/<song>`; a library read from folders alone is the same build with
//! empty tags.

pub mod covers;
pub mod playlists;
pub mod scan;
pub mod sort;
pub mod tags;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::library::sort::sort_key;

use crate::library::playlists::Playlist;

use crate::library::tags::Tags;
use crate::text;

pub const MUSIC_DIR: &str = "Music";
/// Audiobooks, apart from the music: a folder, or a file, a book.
pub const BOOKS_DIR: &str = "Audiobooks";
/// Where the library is read from: the music's folder and the books', as `music.txt` names
/// them, each from the card's root unless a whole path, as on another card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dirs {
    pub music: PathBuf,
    pub books: PathBuf,
}

impl Dirs {
    /// The folders `music` and `books` name, from `root`.
    pub fn named(root: &Path, music: &str, books: &str) -> Dirs {
        Dirs {
            music: root.join(music),
            books: root.join(books),
        }
    }

    /// The folders slot looks in unless told otherwise: `Music` and `Audiobooks` on the card.
    #[cfg(test)]
    pub fn on(root: &Path) -> Dirs {
        Dirs::named(root, MUSIC_DIR, BOOKS_DIR)
    }
}

/// How many albums Recently Added lists.
const RECENT_ALBUMS: usize = 25;

/// What a track, album, artist or book is known by: its place in the library's list of them,
/// which a rescan renumbers. Each kind is its own type, so one is never taken for another, and
/// indexes only its own list.
macro_rules! ids {
    ($($id:ident => $item:ty),* $(,)?) => {$(
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

ids!(
    TrackId => Track,
    AlbumId => Album,
    ArtistId => Artist,
    BookId => Book,
    GenreId => Genre,
    FolderId => Folder,
);

/// The files listed, those slot-player decodes: MP3; AAC or Apple Lossless in MP4 (.m4a, an
/// .m4b book) or alone; FLAC; Vorbis, Opus or FLAC in Ogg (.ogg, .opus); WAV.
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

/// A genre, as the songs' tags name it, and its albums by name.
pub struct Genre {
    pub name: String,
    pub albums: Vec<AlbumId>,
}

/// A folder of Music as it is on the card: its name, and the folders and songs in it, each by
/// name; and, of all in it however deep, how many songs, and the first, its own before its
/// folders', for its picture.
#[derive(Default)]
pub struct Folder {
    pub name: String,
    pub folders: Vec<FolderId>,
    pub tracks: Vec<TrackId>,
    pub songs: usize,
    pub first: Option<TrackId>,
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
    /// The genres the songs are tagged with, by name.
    pub genres: Vec<Genre>,
    /// The albums that say when they came out, oldest first: Years, by decade.
    pub albums_by_year: Vec<AlbumId>,
    /// Music's folders as the songs lie in them, the first Music itself: Folders.
    pub folders: Vec<Folder>,
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
    genre: String,
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
        let or = |tag: &str, fallback: &str| {
            if tag.is_empty() {
                fallback.to_string()
            } else {
                tag.to_string()
            }
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
            genre: r.tags.genre.clone(),
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
    #[cfg(test)]
    pub fn scan(root: &Path) -> Library {
        let progress = crate::library::scan::Progress::default();
        crate::library::scan::scan(root, &Dirs::on(root), true, &progress).0
    }

    /// The library from `records`, in any order, with the playlists in the music's folder and
    /// the audiobooks after it.
    pub fn build(dirs: &Dirs, records: Vec<Record>) -> Library {
        let (music, books) = (&dirs.music, &dirs.books);
        let (book_records, records): (Vec<Record>, Vec<Record>) =
            records.into_iter().partition(|r| r.path.starts_with(books));
        // Album artist, then album, each by its name in any case.
        type Albums = HashMap<String, (String, Vec<Song>)>;
        let mut grouped: HashMap<String, (String, Albums)> = HashMap::new();
        let shared = shared_artists(&records);
        for song in records.into_iter().map(|r| {
            let shared = shared.get(&album_key(&r)).cloned();
            let mut song = Song::of(music, r);
            song.album_artist = shared.unwrap_or(song.album_artist);
            song
        }) {
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
        // Each album's genres, as its songs name them, gathered into the library's once all are in.
        let mut album_genres: Vec<(AlbumId, Vec<String>)> = Vec::new();
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
                .map(|(name, songs)| {
                    let genres = split_genres(songs.iter().map(|s| s.genre.as_str()));
                    let album = lib.add_album(name, artist, songs);
                    album_genres.push((album, genres));
                    album
                })
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
        lib.albums_by_year = (0..lib.albums.len()).map(AlbumId).collect();
        lib.albums_by_year.retain(|&a| lib.albums[a].year > 0);
        lib.albums_by_year
            .sort_by_cached_key(|&a| (lib.albums[a].year, sort_key(&lib.albums[a].name)));
        lib.genres = genres(&lib, &album_genres);
        lib.folders = folders(&lib, music);
        lib.add_books(books, book_records);
        lib.playlists = crate::store::playlists::read(music, &lib);
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

    /// Every song, artist by artist, as Artists plays them; a book's parts are not songs.
    /// The songs in `folder` and the folders in it, in the order it lists them: its folders',
    /// then its own.
    pub fn folder_tracks(&self, folder: FolderId) -> Vec<TrackId> {
        let folder = &self.folders[folder];
        let mut tracks: Vec<TrackId> = folder
            .folders
            .iter()
            .flat_map(|&f| self.folder_tracks(f))
            .collect();
        tracks.extend(&folder.tracks);
        tracks
    }

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

    /// Whether `album` has songs by others than its artist, as a compilation or a guest has.
    pub fn mixed(&self, album: AlbumId) -> bool {
        let by = &self.artists[self.albums[album].artist].name;
        self.albums[album]
            .tracks
            .iter()
            .any(|&t| self.tracks[t].artist != *by)
    }

    /// The album's artist, under whom the song is filed.
    pub fn artist_of(&self, track: TrackId) -> &Artist {
        &self.artists[self.albums[self.tracks[track].album].artist]
    }

    pub fn album_of(&self, track: TrackId) -> &Album {
        &self.albums[self.tracks[track].album]
    }
}

/// Music's folders, from where its songs lie: each folder's folders by name, then its songs by
/// file name, as a file browser lists them.
fn folders(lib: &Library, music: &Path) -> Vec<Folder> {
    let mut folders = vec![Folder::default()];
    let mut known: HashMap<PathBuf, FolderId> = HashMap::new();
    for (i, track) in lib.tracks.iter().enumerate() {
        let Ok(rel) = track.path.strip_prefix(music) else {
            continue;
        };
        let mut here = FolderId(0);
        let mut path = PathBuf::new();
        for part in rel.parent().into_iter().flat_map(Path::components) {
            path.push(part);
            here = match known.get(&path) {
                Some(&folder) => folder,
                None => {
                    let folder = FolderId(folders.len());
                    let name = part.as_os_str().to_string_lossy().into_owned();
                    folders.push(Folder {
                        name,
                        ..Folder::default()
                    });
                    folders[here].folders.push(folder);
                    known.insert(path.clone(), folder);
                    folder
                }
            };
        }
        folders[here].tracks.push(TrackId(i));
    }
    let names: Vec<String> = folders.iter().map(|f| f.name.to_lowercase()).collect();
    let file = |t: &TrackId| {
        lib.tracks[*t]
            .path
            .file_name()
            .map(|n| n.to_ascii_lowercase())
    };
    for folder in &mut folders {
        folder.folders.sort_by(|a, b| names[a.0].cmp(&names[b.0]));
        folder.tracks.sort_by_cached_key(file);
    }
    // A folder comes before those in it, so from the last back each has its own counted.
    for i in (0..folders.len()).rev() {
        let inner = folders[i]
            .folders
            .iter()
            .map(|&f| (folders[f.0].songs, folders[f.0].first));
        let (songs, first): (Vec<usize>, Vec<Option<TrackId>>) = inner.unzip();
        let own = &folders[i].tracks;
        let first = own.first().copied().or(first.into_iter().flatten().next());
        (folders[i].songs, folders[i].first) = (own.len() + songs.iter().sum::<usize>(), first);
    }
    folders
}

/// The genres `tags` name, each once in any case, in the order first named: a tag may name
/// several, as "Rock; Pop" or "Rock/Pop" do.
fn split_genres<'a>(tags: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut genres: Vec<String> = Vec::new();
    for genre in tags.flat_map(|t| t.split([';', '/', '\0'])).map(str::trim) {
        if !genre.is_empty() && !genres.iter().any(|g| g.eq_ignore_ascii_case(genre)) {
            genres.push(genre.to_string());
        }
    }
    genres
}

/// The genres of `albums`, each with its albums by name: one genre for names alike in any
/// case, spelled as the first album to name it does, a capital first letter winning.
fn genres(lib: &Library, albums: &[(AlbumId, Vec<String>)]) -> Vec<Genre> {
    let mut by_name: HashMap<String, Genre> = HashMap::new();
    for (album, names) in albums {
        for name in names {
            let genre = by_name.entry(name.to_lowercase()).or_insert_with(|| Genre {
                name: name.clone(),
                albums: Vec::new(),
            });
            let capital = |s: &str| s.starts_with(char::is_uppercase);
            if capital(name) && !capital(&genre.name) {
                genre.name = name.clone();
            }
            genre.albums.push(*album);
        }
    }
    let mut genres: Vec<Genre> = by_name.into_values().collect();
    genres.sort_by_cached_key(|g| sort_key(&g.name));
    for genre in &mut genres {
        genre
            .albums
            .sort_by_cached_key(|&a| sort_key(&lib.albums[a].name));
    }
    genres
}

/// What finds an album's songs together before their artists are known: their folder, and
/// their album's name in any case.
fn album_key(r: &Record) -> (PathBuf, String) {
    let folder = r.path.parent().map(Path::to_path_buf).unwrap_or_default();
    (folder, r.tags.album.to_lowercase())
}

/// The artist each album with no album artist tags but songs by several is filed under, as
/// players file one: the artist most of its songs share, so a song "feat." another stays on
/// it; else Various Artists, a compilation.
fn shared_artists(records: &[Record]) -> HashMap<(PathBuf, String), String> {
    let mut albums: HashMap<(PathBuf, String), Vec<&Record>> = HashMap::new();
    for r in records.iter().filter(|r| !r.tags.album.is_empty()) {
        albums.entry(album_key(r)).or_default().push(r);
    }
    albums
        .into_iter()
        .filter(|(_, songs)| songs.iter().all(|r| r.tags.album_artist.is_empty()))
        .filter_map(|(key, songs)| {
            let mut counts: HashMap<String, (usize, &str)> = HashMap::new();
            for r in songs.iter().filter(|r| !r.tags.artist.is_empty()) {
                let artist = &r.tags.artist;
                counts.entry(artist.to_lowercase()).or_insert((0, artist)).0 += 1;
            }
            if counts.len() < 2 {
                return None;
            }
            let (most, artist) = counts
                .into_values()
                .max_by_key(|&(n, a)| (n, std::cmp::Reverse(a)))?;
            let shared = if most * 2 > songs.len() {
                artist.to_string()
            } else {
                text::VARIOUS_ARTISTS.to_string()
            };
            Some((key, shared))
        })
        .collect()
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
    if numbered && !title.is_empty() && title.len() < rest.len() {
        title.to_string()
    } else {
        stem.to_string()
    }
}
