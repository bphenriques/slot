//! Reading the card: songs, tags, folders, playlists and the cache.

use super::common::*;

#[test]
fn untagged_songs_are_filed_by_their_folders_in_shelf_order() {
    let (_d, lib) = library(&[
        "Aerosmith/Toys in the Attic/02 Uncle Salty.mp3",
        "Aerosmith/Toys in the Attic/01 Toys in the Attic.mp3",
        "Aerosmith/Get a Grip/01 Intro.flac",
        "3 Doors Down/Away from the Sun/01 When I'm Gone.mp3",
        "Air Traffic/cover.jpg",
        "Loose.mp3",
        ".hidden/x/1.mp3",
    ]);
    // A song outside any folder is still a song, by an artist no folder names.
    assert_eq!(names(&lib), ["3 Doors Down", "Aerosmith", "Unknown Artist"]);
    let aerosmith = &lib.artists[1];
    let albums: Vec<_> = aerosmith
        .albums
        .iter()
        .map(|&a| lib.albums[a].name.as_str())
        .collect();
    assert_eq!(albums, ["Get a Grip", "Toys in the Attic"]);
    let toys = &lib.albums[aerosmith.albums[1]];
    let tracks: Vec<_> = toys
        .tracks
        .iter()
        .map(|&t| lib.tracks[t].title.as_str())
        .collect();
    assert_eq!(tracks, ["Toys in the Attic", "Uncle Salty"]);
}

#[test]
fn a_track_reads_without_its_number_unless_the_number_is_the_title() {
    assert_eq!(track_title("03 Dream On.mp3"), "Dream On");
    assert_eq!(track_title("1-03 - Dream On.flac"), "Dream On");
    assert_eq!(track_title("Dream On.mp3"), "Dream On");
    assert_eq!(track_title("1999.mp3"), "1999");
    assert_eq!(track_title("2Pac.mp3"), "2Pac");
}

#[test]
fn update_library_reads_what_changed_and_held_reads_everything_again() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    // Settings, then up round to Advanced, the last.
    press(&mut b, &lib, &[Key::Up, Key::A, Key::Up, Key::A]);
    assert_eq!(b.view(), View::Advanced, "the Advanced page, last");
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let rows: Vec<&str> = page.rows.iter().map(|r| r.text.as_str()).collect();
    let measured = cfg!(feature = "perf").then_some("Performance Log");
    let want: Vec<&str> = ["Scrobbler Log"]
        .into_iter()
        .chain(measured)
        .chain(["Detailed Log", "Experimental"])
        .collect();
    assert_eq!(
        rows, want,
        "the scrobbler log, the logs, then what is still being shaped"
    );
    press(&mut b, &lib, &[Key::B]);
    // Up past Help to Update Library: tapped it updates, held it rebuilds.
    press(&mut b, &lib, &[Key::Up, Key::Up]);
    let cx = Context::new(&lib, at(0));
    assert_eq!(b.row(&cx), Some(crate::Row::Rescan));
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    assert_eq!(b.held(&cx, Key::A), Outcome::Rescan(true));
    assert_eq!(press(&mut b, &lib, &[Key::A]), Outcome::Rescan(false));
    let legend = crate::text::legend(crate::screens::legend(&b, &cx));
    let a = legend.iter().find(|h| h.key == "A").unwrap();
    assert_eq!(a.label(), "Update (Rebuild)");
}

#[test]
fn a_track_keeps_the_number_its_file_starts_with() {
    assert_eq!(crate::track_number("03 Dream On.mp3"), "03");
    assert_eq!(crate::track_number("1-03 - Dream On.flac"), "1-03");
    assert_eq!(crate::track_number("Dream On.mp3"), "");
    assert_eq!(crate::track_number("1999.mp3"), "");
}

#[test]
fn after_a_rescan_the_track_playing_plays_on_without_asking_the_player_again() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 1, None));
    p.forget();
    assert!(
        p.current().is_none() && p.owns_audio(),
        "the player keeps the codec meanwhile"
    );
    p.restore(Queue::new(ids(&[0, 1]), 1, None), 30.0, true);
    assert_eq!(p.current(), Some(TrackId(1)));
    assert!(!p.resumable() && p.owns_audio());
    assert_eq!(
        p.toggle(&lib),
        Some(Command::Pause),
        "it is playing, not waiting to"
    );
}

#[test]
fn playlists_read_their_songs_by_path_with_favorites_always_first() {
    let (d, _) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "Y/B/01 three.mp3"]);
    let lists = d.path().join("Music/Playlists");
    std::fs::create_dir_all(&lists).unwrap();
    // From its own folder, from the music folder, from the card, and a song not there.
    let text = "#EXTM3U\n../Y/B/01 three.mp3\nX/A/01 one.mp3\nMusic/X/A/02 two.mp3\ngone.mp3\n";
    std::fs::write(lists.join("Road Trip.m3u8"), text).unwrap();
    let lib = Library::scan(d.path());
    assert_eq!(
        names(&lib),
        ["X", "Y"],
        "the playlists folder is not an artist"
    );
    let read: Vec<(&str, &[TrackId])> = lib
        .playlists
        .iter()
        .map(|p| (p.name.as_str(), p.tracks.as_slice()))
        .collect();
    assert_eq!(
        read,
        [("Favorites", &[][..]), ("Road Trip", &ids(&[2, 0, 1])[..])]
    );
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::Down, Key::A]);
    assert_eq!(b.view(), View::Playlists);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(
        page.rows,
        [
            "Favorites",
            "Recently Played",
            "Recently Added",
            "Road Trip"
        ]
    );
    assert_eq!(page.values[3].as_deref(), Some("3"), "its songs counted");
}

#[test]
fn recently_played_is_kept_by_path_and_listed_with_the_playlists() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    let mut recent = Recent::default();
    recent.hear(TrackId(0), 30.0, 60.0, true);
    recent.hear(TrackId(1), 30.0, 60.0, true);
    let text = recent.encode(card);
    assert_eq!(
        text,
        "#EXTM3U\nMusic/X/A/02 two.mp3\nMusic/X/A/01 one.mp3\n"
    );
    std::fs::remove_file(d.path().join("Music/X/A/02 two.mp3")).unwrap();
    let lib = Library::scan(d.path());
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    assert_eq!(
        Recent::decode(&text, card).tracks,
        ids(&[0]),
        "a song gone is left out"
    );
    let cx = Context {
        recent: &[TrackId(0)],
        ..Context::new(&lib, at(0))
    };
    let mut b = Browser::default();
    // Music menu: Now Playing, Queue, Library, Playlists; then Favorites, Recently Played.
    for key in [Key::Down, Key::Down, Key::Down, Key::A, Key::Down, Key::A] {
        b.press(&cx, key);
    }
    assert_eq!(b.view(), View::RecentlyPlayed);
    assert_eq!(b.page(&cx).rows, ["Play All", "one"]);
    // A song there plays from it, as in any list of songs.
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    let play = b.press(&cx, Key::A);
    assert!(matches!(play, Outcome::Play { start: 0, .. }), "{play:?}");
}

#[test]
fn favorites_is_written_as_a_playlist_other_players_read() {
    let (d, _) = library(&["X/A/01 one.mp3"]);
    let mut lib = Library::scan(d.path());
    lib.playlists[0].tracks.push(TrackId(0));
    let store = Store::open(d.path());
    store.write_playlist(&lib.playlists[0], &lib);
    store.flush();
    let text = std::fs::read_to_string(d.path().join("Music/Playlists/Favorites.m3u8")).unwrap();
    assert_eq!(text, "#EXTM3U\n../X/A/01 one.mp3\n");
    assert_eq!(Library::scan(d.path()).playlists[0].tracks, ids(&[0]));
}

#[test]
fn tags_come_first_and_folders_fill_in_what_they_leave_out() {
    use crate::Tags;
    let lib = Library::build(
        Path::new("/card"),
        vec![
            // A compilation: one album, filed under its album artist.
            record(
                "Comp/Hits/02 b.mp3",
                Tags {
                    title: "Second".into(),
                    artist: "Bea".into(),
                    album_artist: "Various Artists".into(),
                    album: "Hits".into(),
                    disc: 1,
                    track: 2,
                    ..crate::Tags::default()
                },
            ),
            record(
                "Comp/Hits/01 a.mp3",
                Tags {
                    title: "First".into(),
                    artist: "Al".into(),
                    album_artist: "Various Artists".into(),
                    album: "Hits".into(),
                    disc: 1,
                    track: 1,
                    ..crate::Tags::default()
                },
            ),
            // Two discs, ordered by disc then track whatever the file names say.
            record(
                "X/Big/a.mp3",
                Tags {
                    title: "Two".into(),
                    album: "Big".into(),
                    disc: 2,
                    track: 1,
                    ..crate::Tags::default()
                },
            ),
            record(
                "X/Big/b.mp3",
                Tags {
                    title: "One".into(),
                    album: "Big".into(),
                    disc: 1,
                    track: 5,
                    ..crate::Tags::default()
                },
            ),
        ],
    );
    assert_eq!(names(&lib), ["Various Artists", "X"]);
    let titles = |a: usize| -> Vec<(&str, &str, &str)> {
        lib.albums[a]
            .tracks
            .iter()
            .map(|&t| {
                let t = &lib.tracks[t];
                (t.number.as_str(), t.title.as_str(), t.artist.as_str())
            })
            .collect()
    };
    assert_eq!(titles(0), [("01", "First", "Al"), ("02", "Second", "Bea")]);
    assert_eq!(
        titles(1),
        [("1-05", "One", "X"), ("2-01", "Two", "X")],
        "the artist from its folder; the disc in its number"
    );
}

#[test]
fn a_scan_reads_only_songs_that_changed_since_the_cache() {
    let (d, _) = library(&["X/A/01 one.mp3"]);
    let progress = crate::Progress::default();
    crate::scan_library(d.path(), false, &progress);
    let cache = d.path().join(crate::CACHE_FILE);
    // Tags only the cache knows: an unchanged song keeps them, a full scan does not.
    let text = std::fs::read_to_string(&cache).unwrap();
    std::fs::write(&cache, text.replace("\t\t\t\t\t", "\tCached\t\t\t\t")).unwrap();
    // The cache's library comes at once, without the card; the scan then finds it the same.
    let shown = crate::cached_library(d.path()).unwrap();
    assert_eq!(shown.tracks[0].title, "Cached");
    let (lib, changed) = crate::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("Cached", false));
    let (lib, changed) = crate::scan_library(d.path(), true, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("one", true));
    // A changed song is read again.
    std::fs::write(&cache, text.replace("\t\t\t\t\t", "\tCached\t\t\t\t")).unwrap();
    std::fs::write(d.path().join("Music/X/A/01 one.mp3"), b"changed").unwrap();
    let (lib, changed) = crate::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("one", true));
    // A song gone is a change too.
    std::fs::remove_file(d.path().join("Music/X/A/01 one.mp3")).unwrap();
    let (lib, changed) = crate::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks.len(), changed), (0, true));
}

#[test]
fn recently_added_is_by_when_the_library_got_them_and_an_album_artist_names_the_song() {
    use crate::Tags;
    let added = |path: &str, added: u64, modified: u64, tags: Tags| crate::Record {
        added,
        modified,
        ..record(path, tags)
    };
    let lib = Library::build(
        Path::new("/card"),
        vec![
            // Copied in last, though its files are the oldest.
            added("A/Old/01 a.mp3", 300, 10, Tags::default()),
            added("A/New/01 b.mp3", 100, 90, Tags::default()),
            added("A/Same/01 c.mp3", 100, 50, Tags::default()),
            added(
                "Loose.mp3",
                0,
                0,
                Tags {
                    album_artist: "Band".into(),
                    ..crate::Tags::default()
                },
            ),
        ],
    );
    let recent: Vec<&str> = lib
        .albums_by_added
        .iter()
        .map(|&a| lib.albums[a].name.as_str())
        .collect();
    assert_eq!(recent, ["Old", "New", "Same", "Unknown Album"]);
    let loose = lib.tracks.iter().find(|t| t.title == "Loose").unwrap();
    assert_eq!(loose.artist, "Band", "not Unknown Artist");
}

#[test]
fn an_older_m3u_reads_in_latin_1_with_windows_folders() {
    let (d, _) = library(&["Café/A/01 x.mp3", "B/C/01 y.mp3"]);
    let lists = d.path().join("Music/Playlists");
    std::fs::create_dir_all(&lists).unwrap();
    // "Café" in Latin-1, as Windows players wrote .m3u files, with their backslashes.
    std::fs::write(
        lists.join("Old.m3u"),
        b"#EXTM3U\r\n..\\Caf\xe9\\A\\01 x.mp3\r\n",
    )
    .unwrap();
    // A UTF-8 file with a byte order mark before its first song.
    std::fs::write(lists.join("New.m3u8"), "\u{feff}../B/C/01 y.mp3\n").unwrap();
    let lib = Library::scan(d.path());
    let found: Vec<(&str, usize)> = lib
        .playlists
        .iter()
        .map(|p| (p.name.as_str(), p.tracks.len()))
        .collect();
    assert_eq!(found, [("Favorites", 0), ("New", 1), ("Old", 1)]);
}

#[test]
fn an_artist_lists_albums_by_year_and_discs_are_ruled_apart() {
    use crate::Tags;
    let tagged = |path: &str, album: &str, year: u32, disc: u32, track: u32| {
        record(
            path,
            Tags {
                artist: "X".into(),
                album: album.into(),
                year,
                disc,
                track,
                ..crate::Tags::default()
            },
        )
    };
    let lib = Library::build(
        Path::new("/card"),
        vec![
            tagged("X/A/1.mp3", "Arrival", 1999, 1, 1),
            tagged("X/B/1.mp3", "Begin", 1990, 1, 1),
            tagged("X/C/1.mp3", "Undated", 0, 1, 1),
            tagged("X/D/1.mp3", "Double", 2005, 1, 1),
            tagged("X/D/2.mp3", "Double", 2005, 2, 1),
        ],
    );
    let albums: Vec<&str> = lib.artists[0]
        .albums
        .iter()
        .map(|&a| lib.albums[a].name.as_str())
        .collect();
    assert_eq!(albums, ["Begin", "Arrival", "Double", "Undated"]);
    let double = lib.artists[0].albums[2];
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    let row = lib
        .albums_by_name
        .iter()
        .position(|&a| a == double)
        .unwrap();
    for _ in 0..row {
        press(&mut b, &lib, &[Key::Down]);
    }
    press(&mut b, &lib, &[Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rules, [1, 2], "under Play All, then disc 2");
}

#[test]
fn a_song_s_own_tags_name_it_file_it_and_say_how_long_it_plays() {
    use lofty::prelude::*;
    use lofty::tag::{ItemKey, Tag, TagType};
    let (d, _) = library(&[]);
    // A second of silence, as a WAV, tagged as a tagger would.
    let path = d.path().join("Music/loose/track.wav");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (rate, frames) = (8_000u32, 8_000u32);
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + frames * 2).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend([1u16, 1].iter().flat_map(|v| v.to_le_bytes()));
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend([2u16, 16].iter().flat_map(|v| v.to_le_bytes()));
    wav.extend(b"data");
    wav.extend((frames * 2).to_le_bytes());
    wav.resize(wav.len() + frames as usize * 2, 0);
    std::fs::write(&path, wav).unwrap();
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title("Hey Mama".into());
    tag.set_artist("Black Eyed Peas feat. Tippa Irie".into());
    tag.insert_text(ItemKey::AlbumArtist, "Black Eyed Peas".into());
    tag.set_album("Elephunk".into());
    tag.set_track(7);
    tag.set_disk(1);
    tag.insert_text(ItemKey::RecordingDate, "2003".into());
    tag.save_to_path(&path, lofty::config::WriteOptions::default())
        .unwrap();
    // The same, as a book's part: a book is timed, for how far into it the listener is.
    let part = d
        .path()
        .join("Audiobooks/Black Eyed Peas - Elephunk/track.wav");
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::copy(&path, &part).unwrap();
    let lib = Library::scan(d.path());
    let song = lib.tracks.iter().find(|t| t.book.is_none()).unwrap();
    assert_eq!(song.title, "Hey Mama");
    assert_eq!(
        song.artist, "Black Eyed Peas feat. Tippa Irie",
        "the song's own artist"
    );
    assert_eq!(song.number, "07");
    let album = &lib.albums[song.album];
    assert_eq!((album.name.as_str(), album.year), ("Elephunk", 2003));
    assert_eq!(
        lib.artists[album.artist].name, "Black Eyed Peas",
        "filed by its album artist"
    );
    let part = lib.tracks.iter().find(|t| t.book.is_some()).unwrap();
    assert_eq!(part.secs, 1.0, "a second long");
}

#[test]
fn only_files_the_player_can_play_are_listed() {
    let (_d, lib) = library(&[
        "X/A/01 one.mp3",
        "X/A/02 two.flac",
        "X/A/03 three.m4a",
        "X/A/04 four.ogg",
        "X/A/05 five.wav",
        "X/A/06 six.opus",
        "X/A/07 seven.wma",
    ]);
    let names: Vec<_> = lib.tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(names.len(), 6, "Opus too, not WMA: {names:?}");
    assert!(names.contains(&"six") && !names.contains(&"seven"));
}

/// A song in `folder` of `album`, by `artist`, numbered `n`, as its tags have it.
fn tagged(folder: &str, album: &str, artist: &str, n: u32) -> crate::Record {
    crate::Record {
        path: std::path::Path::new("/card/Music")
            .join(folder)
            .join(format!("{n:02}.mp3")),
        modified: 0,
        size: 0,
        added: 0,
        tags: crate::Tags {
            title: format!("{artist} {n}"),
            artist: artist.into(),
            album: album.into(),
            track: n,
            ..crate::Tags::default()
        },
    }
}

#[test]
fn an_album_by_several_artists_with_no_album_artist_is_one_album_as_players_file_it() {
    let root = std::path::Path::new("/card");
    let lib = Library::build(
        root,
        vec![
            tagged("Hits", "Hits", "Ann", 1),
            tagged("Hits", "Hits", "Bob", 2),
            tagged("Hits", "Hits", "Cat", 3),
            // Most of an album by one artist, one song with a guest: still theirs.
            tagged("Dee/Solo", "Solo", "Dee", 1),
            tagged("Dee/Solo", "Solo", "Dee feat. Ann", 2),
            tagged("Dee/Solo", "Solo", "Dee", 3),
            // Albums alike in name in other folders are not one.
            tagged("Ann/Hits", "Hits", "Ann", 1),
        ],
    );
    let albums: Vec<(&str, &str, usize)> = lib
        .albums
        .iter()
        .map(|a| {
            (
                lib.artists[a.artist].name.as_str(),
                a.name.as_str(),
                a.tracks.len(),
            )
        })
        .collect();
    assert_eq!(
        albums,
        [
            ("Ann", "Hits", 1),
            ("Dee", "Solo", 3),
            (crate::text::VARIOUS_ARTISTS, "Hits", 3)
        ]
    );
    let guest = lib
        .tracks
        .iter()
        .find(|t| t.title == "Dee feat. Ann 2")
        .unwrap();
    assert_eq!(
        guest.artist, "Dee feat. Ann",
        "each song keeps its own artist"
    );
}

/// As `tagged`, of `genre`, out in `year`.
fn of_genre(folder: &str, artist: &str, genre: &str, year: u32) -> crate::Record {
    let mut r = tagged(folder, folder, artist, 1);
    (r.tags.genre, r.tags.year) = (genre.into(), year);
    r
}

#[test]
fn genres_gather_albums_by_name_in_any_case_and_a_tag_may_name_several() {
    let root = std::path::Path::new("/card");
    let lib = Library::build(
        root,
        vec![
            of_genre("Kid A", "Radiohead", "Rock; Electronic", 2000),
            of_genre("OK Computer", "Radiohead", "rock", 1997),
            of_genre("Homogenic", "Bjork", "Electronic/Pop", 1997),
            of_genre("Untagged", "Nobody", "", 0),
        ],
    );
    let genres: Vec<(&str, Vec<&str>)> = lib
        .genres
        .iter()
        .map(|g| {
            let albums = g.albums.iter().map(|&a| lib.albums[a].name.as_str());
            (g.name.as_str(), albums.collect())
        })
        .collect();
    assert_eq!(
        genres,
        [
            ("Electronic", vec!["Homogenic", "Kid A"]),
            ("Pop", vec!["Homogenic"]),
            ("Rock", vec!["Kid A", "OK Computer"]),
        ],
        "by name, each album under each, one Rock for rock"
    );
    let years: Vec<(u32, &str)> = lib
        .albums_by_year
        .iter()
        .map(|&a| (lib.albums[a].year, lib.albums[a].name.as_str()))
        .collect();
    assert_eq!(
        years,
        [(1997, "Homogenic"), (1997, "OK Computer"), (2000, "Kid A")],
        "oldest first, an album that does not say left out"
    );
}

#[test]
fn library_lists_genres_and_years_by_decade_only_when_the_tags_fill_them() {
    let root = std::path::Path::new("/card");
    let lib = Library::build(
        root,
        vec![
            of_genre("Kid A", "Radiohead", "Rock", 2000),
            of_genre("OK Computer", "Radiohead", "Rock", 1997),
            of_genre("Debut", "Bjork", "Pop", 1993),
        ],
    );
    let s = Settings::default();
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), s));
    assert_eq!(
        page.rows,
        ["Artists", "Albums", "Songs", "Genres", "Years", "Folders"]
    );
    // Years: the decades, then a decade's albums by year, played as one.
    press(&mut b, &lib, &[Key::Up, Key::Up, Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), s));
    assert_eq!(page.rows, ["1990–99", "2000–09"]);
    assert_eq!(page.values, [Some("2".to_string()), Some("1".to_string())]);
    press(&mut b, &lib, &[Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), s));
    assert_eq!(page.title, "1990–99");
    let albums: Vec<(&str, &str)> = page.rows[1..]
        .iter()
        .map(|r| (r.text.as_str(), r.after.as_str()))
        .collect();
    assert_eq!(
        albums,
        [
            ("Debut", "Bjork · 1993"),
            ("OK Computer", "Radiohead · 1997")
        ],
        "who made each and when, below, as an artist's albums say when"
    );
    let Outcome::Play { tracks, .. } = press(&mut b, &lib, &[Key::A]) else {
        panic!("Play All plays the decade");
    };
    assert_eq!(tracks.len(), 2);
    // Genres: a genre's albums, which X adds to the queue whole.
    let mut b = Browser::default();
    press(
        &mut b,
        &lib,
        &[
            Key::Down,
            Key::Down,
            Key::A,
            Key::Up,
            Key::Up,
            Key::Up,
            Key::A,
        ],
    );
    assert_eq!(
        b.page(&cx(&lib, QueueView::default(), s)).rows,
        ["Pop", "Rock"]
    );
    let Outcome::Enqueue(tracks) = press(&mut b, &lib, &[Key::Down, Key::X]) else {
        panic!("X adds the genre");
    };
    assert_eq!(tracks.len(), 2);
    // Untagged, neither is offered; the folders always are.
    let (_d, plain) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &plain, &[Key::Down, Key::Down, Key::A]);
    let page = b.page(&cx(&plain, QueueView::default(), s));
    assert_eq!(page.rows, ["Artists", "Albums", "Songs", "Folders"]);
}

#[test]
fn a_cache_from_before_genres_keeps_when_songs_came_in_and_has_their_tags_read_again() {
    let (d, _) = library(&["X/A/01 one.mp3"]);
    let progress = crate::Progress::default();
    crate::scan_library(d.path(), false, &progress);
    let cache = d.path().join(crate::CACHE_FILE);
    let text = std::fs::read_to_string(&cache).unwrap();
    // As the last layout wrote it: no genre, its own header, a title only it knows, and an
    // older day the song came in.
    let line = text.lines().nth(1).unwrap();
    let mut fields: Vec<&str> = line.split('\t').collect();
    fields.truncate(12);
    (fields[3], fields[4]) = ("42", "Cached");
    std::fs::write(&cache, format!("#SLOT-LIBRARY 4\n{}\n", fields.join("\t"))).unwrap();
    let shown = crate::cached_library(d.path()).unwrap();
    assert_eq!(shown.tracks[0].title, "Cached", "shown at once as it was");
    let (lib, changed) = crate::scan_library(d.path(), false, &progress);
    assert_eq!(
        (lib.tracks[0].title.as_str(), changed),
        ("one", true),
        "read again"
    );
    assert_eq!(
        lib.albums[lib.tracks[0].album].added, 42,
        "when it came in, kept"
    );
    let text = std::fs::read_to_string(&cache).unwrap();
    assert!(
        text.starts_with("#SLOT-LIBRARY 5\n"),
        "written in the new layout"
    );
}

#[test]
fn a_compilation_s_songs_say_who_sings_each_and_an_artist_s_own_album_does_not() {
    let root = std::path::Path::new("/card");
    let lib = Library::build(
        root,
        vec![
            tagged("Hits", "Hits", "Ann", 1),
            tagged("Hits", "Hits", "Bob", 2),
            tagged("Hits", "Hits", "Cat", 3),
            tagged("Dee/Solo", "Solo", "Dee", 1),
            tagged("Dee/Solo", "Solo", "Dee feat. Ann", 2),
            tagged("Dee/Solo", "Solo", "Dee", 3),
        ],
    );
    let album = |name: &str| AlbumId(lib.albums.iter().position(|a| a.name == name).unwrap());
    let after = |name: &str| -> Vec<String> {
        let mut b = Browser::default();
        b.push(View::Album(album(name)), 0);
        let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
        page.rows[1..].iter().map(|r| r.after.clone()).collect()
    };
    assert_eq!(after("Hits"), ["Ann", "Bob", "Cat"]);
    assert_eq!(
        after("Solo"),
        ["", "Dee feat. Ann", ""],
        "only the song with a guest"
    );
}

#[test]
fn the_shoulders_jump_a_letter_among_the_playlists_after_those_pinned() {
    let (d, _) = library(&["X/A/01 one.mp3"]);
    let lists = d.path().join("Music/Playlists");
    std::fs::create_dir_all(&lists).unwrap();
    for name in ["Bike Ride", "Cooking", "Road Trip"] {
        std::fs::write(lists.join(format!("{name}.m3u")), "X/A/01 one.mp3\n").unwrap();
    }
    let lib = Library::scan(d.path());
    let cx = cx(&lib, QueueView::default(), Settings::default());
    let mut b = Browser::default();
    b.push(View::Playlists, 3);
    assert_eq!(b.page(&cx).rows[3], "Bike Ride");
    b.press(&cx, Key::R1);
    assert_eq!(b.page(&cx).rows[b.at()], "Cooking", "the next letter");
}

#[test]
fn folders_list_their_folders_by_name_then_their_songs_by_file_name_as_on_the_card() {
    use crate::library::FolderId;
    let (_d, lib) = library(&[
        "b.mp3",
        "Zed/2 two.mp3",
        "Zed/1 one.mp3",
        "abba/Gold/01 x.mp3",
        "../Audiobooks/Book/01 a.mp3",
    ]);
    let file = |t: &TrackId| lib.tracks[*t].path.file_name().unwrap().to_string_lossy();
    let files = |ts: &[TrackId]| ts.iter().map(file).collect::<Vec<_>>();
    let root = &lib.folders[FolderId(0)];
    let names: Vec<&str> = root
        .folders
        .iter()
        .map(|&f| lib.folders[f].name.as_str())
        .collect();
    assert_eq!(names, ["abba", "Zed"], "by name, in any case");
    assert_eq!(
        files(&root.tracks),
        ["b.mp3"],
        "a song in Music itself; no book"
    );
    assert_eq!(
        files(&lib.folders[root.folders[1]].tracks),
        ["1 one.mp3", "2 two.mp3"]
    );
    assert_eq!(
        files(&lib.folder_tracks(FolderId(0))),
        ["01 x.mp3", "1 one.mp3", "2 two.mp3", "b.mp3"],
        "under a folder: its folders' songs, then its own"
    );
}

#[test]
fn an_opus_file_is_filed_by_its_own_tags() {
    let opus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../slot-player/tests/fixtures/tone.opus");
    let d = tempfile::tempdir().unwrap();
    let to = d.path().join("Music/any/where.opus");
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::copy(opus, &to).unwrap();
    let lib = Library::scan(d.path());
    let t = &lib.tracks[TrackId(0)];
    let album = &lib.albums[t.album];
    assert_eq!(
        (t.title.as_str(), album.name.as_str()),
        ("Tone", "Fixtures")
    );
    assert_eq!(t.artist, "Slot");
}
