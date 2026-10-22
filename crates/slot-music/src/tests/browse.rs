//! Moving through the screens: lists, menus, their keys and hints.

use super::common::*;

#[test]
fn the_main_screen_keeps_its_rows_whether_or_not_anything_plays() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let b = Browser::default();
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(
        page.rows,
        [
            "Shuffle Library",
            "Queue",
            "Library",
            "Playlists",
            "Audiobooks",
            "Settings"
        ]
    );
    assert_eq!(page.pages, 1, "everything fits one page");
}

#[test]
fn play_all_on_an_artist_queues_every_album_in_order() {
    let (_d, lib) = library(&[
        "Aerosmith/B/01 b1.mp3",
        "Aerosmith/A/01 a1.mp3",
        "Aerosmith/A/02 a2.mp3",
    ]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    let out = press(&mut b, &lib, &[Key::A, Key::A]);
    let Outcome::Play {
        tracks,
        start,
        shuffle,
    } = out
    else {
        panic!("{out:?}");
    };
    let titles: Vec<_> = tracks
        .iter()
        .map(|&t| lib.tracks[t].title.as_str())
        .collect();
    assert_eq!(titles, ["a1", "a2", "b1"]);
    assert_eq!((start, shuffle), (0, crate::Shuffle::Off));
    assert_eq!(b.view(), View::NowPlaying);
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Artist(ArtistId(0)));
}

#[test]
fn a_track_in_an_album_plays_from_there_to_the_end_of_the_album() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    // A -> (Play All, one, two) two
    let out = press(&mut b, &lib, &[Key::A, Key::Down, Key::Down, Key::A]);
    assert_eq!(
        out,
        Outcome::Play {
            tracks: lib.albums[0].tracks.clone(),
            start: 1,
            shuffle: crate::Shuffle::Off
        }
    );
}

#[test]
fn shoulders_jump_by_letter_and_left_right_by_page() {
    let artists = [
        "Abba", "Air", "Beck", "Blur", "Bjork", "Cake", "Doves", "Eels", "Fugazi",
    ];
    let files: Vec<String> = artists.iter().map(|a| format!("{a}/X/1 t.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    let at = |b: &Browser| {
        let p = b.page(&cx(&lib, QueueView::default(), Settings::default()));
        p.rows[p.selected].clone()
    };
    press(&mut b, &lib, &[Key::R1]);
    assert_eq!(at(&b), "Beck");
    press(&mut b, &lib, &[Key::Down, Key::R1]);
    assert_eq!(at(&b), "Cake");
    press(&mut b, &lib, &[Key::L1]);
    assert_eq!(at(&b), "Beck");
    press(&mut b, &lib, &[Key::L1]);
    assert_eq!(at(&b), "Abba");
    press(&mut b, &lib, &[Key::Right]);
    assert_eq!(at(&b), artists[crate::browse::LIST_ROWS]);
    press(&mut b, &lib, &[Key::Left]);
    assert_eq!(at(&b), "Abba");
}

#[test]
fn b_backs_out_one_level_and_leaves_from_the_top() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(b.view(), View::Artists);
    assert_eq!(press(&mut b, &lib, &[Key::B]), Outcome::None, "to Library");
    assert_eq!(
        press(&mut b, &lib, &[Key::B]),
        Outcome::None,
        "to the Music menu"
    );
    assert_eq!(press(&mut b, &lib, &[Key::B]), Outcome::Leave);
}

#[test]
fn x_adds_what_a_row_stands_for_to_the_queue_and_a_held_plays_it_next() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(
        press(&mut b, &lib, &[Key::X]),
        Outcome::Enqueue(ids(&[0, 1, 2]))
    );
    // X -> (Play All, A, B): album B
    press(&mut b, &lib, &[Key::A, Key::Down, Key::Down]);
    let out = press(&mut b, &lib, &[Key::X]);
    assert_eq!(out, Outcome::Enqueue(lib.albums[1].tracks.clone()));
    assert_eq!(b.view(), View::Artist(ArtistId(0)), "adding is not opening");
    let cx = Context::new(&lib, at(0));
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    assert_eq!(
        b.held(&cx, Key::A),
        Outcome::PlayNext(lib.albums[1].tracks.clone())
    );
    assert_eq!(b.view(), View::Artist(ArtistId(0)), "nor is putting next");
}

#[test]
fn select_goes_to_now_playing_then_the_queue_and_back() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(press(&mut b, &lib, &[Key::Select]), Outcome::None);
    assert_eq!(b.view(), View::NowPlaying);
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::Queue);
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Artists, "back to where it was");
}

#[test]
fn select_with_a_shoulder_skips_from_any_music_screen() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(press(&mut b, &lib, &[Key::SelectR1]), Outcome::Skip(1));
    assert_eq!(press(&mut b, &lib, &[Key::SelectL1]), Outcome::Skip(-1));
    assert_eq!(
        b.view(),
        View::Artists,
        "a skip leaves the list where it was"
    );
}

#[test]
fn tracks_play_or_shuffle_everything_from_their_top_rows() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    assert_eq!(b.view(), View::Tracks);
    // Play All: tapped it plays, held it shuffles.
    let cx = cx(&lib, QueueView::default(), Settings::default());
    let play = |shuffle| Outcome::Play {
        tracks: lib.tracks_by_title.clone(),
        start: 0,
        shuffle,
    };
    assert_eq!(b.held(&cx, Key::A), play(crate::Shuffle::Tracks));
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    assert_eq!(press(&mut b, &lib, &[Key::A]), play(crate::Shuffle::Off));
}

#[test]
fn only_a_opens_a_row_in_a_list() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let view = b.view();
    for key in [Key::L2, Key::R2] {
        assert_eq!(press(&mut b, &lib, &[key]), Outcome::None, "{key:?}");
        assert_eq!(b.view(), view, "{key:?}");
    }
}

#[test]
fn lists_count_their_items_and_not_the_rows_above_them() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let page = |b: &Browser| b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!((page(&b).item, page(&b).items), (None, 3), "on Play All");
    press(&mut b, &lib, &[Key::Down, Key::Down]);
    assert_eq!((page(&b).item, page(&b).items), (Some(2), 3));
}

#[test]
fn rows_name_their_context_dim_and_mark_what_plays() {
    let (_d, lib) = library(&[
        "Aerosmith/Get a Grip/03 Cryin.mp3",
        "Blur/Parklife/01 Girls.mp3",
    ]);
    let settings = Settings::default();
    let current = ids(&[0]);
    let queue = QueueView {
        tracks: &current,
        at: 0,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let mut b = Browser::default();
    // The albums of everyone name whose they are.
    press(&mut b, &lib, &TO_ALBUMS);
    let page = b.page(&cx(&lib, queue, settings));
    assert_eq!(
        (page.rows[1].text.as_str(), page.rows[1].after.as_str()),
        ("Get a Grip", "Aerosmith")
    );
    // An album's own tracks go by their numbers, and what plays is marked.
    b.press(&cx(&lib, queue, settings), Key::A);
    let page = b.page(&cx(&lib, queue, settings));
    let row = &page.rows[1];
    // Its number kept, for the mark to stand in its room, the name in line with the rest.
    assert_eq!((row.before.as_str(), row.text.as_str()), ("03", "Cryin"));
    assert!(row.playing && row.after.is_empty());
    assert!(page.rows[0].action && !row.action && !page.rows[0].playing);
}

#[test]
fn the_main_screen_counts_what_its_rows_lead_to() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/B/01 two.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let values: Vec<Option<&str>> = page.values.iter().map(Option::as_deref).collect();
    assert_eq!(
        values,
        [Some("1"), Some("2"), Some("2"), Some("1")],
        "Library: Artists, Albums, Songs, and Folders by what is in it; no Genres or Years with none tagged"
    );
}

#[test]
fn a_letter_jump_says_where_it_landed() {
    let files = ["Abba/X/1 t.mp3", "Blur/X/1 t.mp3"];
    let (_d, lib) = library(&files);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(press(&mut b, &lib, &[Key::R1]), Outcome::Letter('B'));
}

#[test]
fn the_menu_goes_to_the_album_on_the_track_and_a_held_in_a_list_plays_next() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let tracks = ids(&[1]);
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, s), Key::Select);
    b.press(&cx(&lib, queue, s), Key::Start);
    assert_eq!(b.view(), View::PlayingMenu);
    let page = b.page(&cx(&lib, queue, s));
    assert_eq!(
        page.rows,
        [
            "Sleep Timer",
            "Repeat",
            "Shuffle",
            "Go to Album",
            "Go to Artist",
            "Song Info",
        ],
        "what to set first, then where to go; Y favorites, so the menu does not"
    );
    assert_eq!(page.values[2].as_deref(), Some("Off"), "shuffle says which");
    for _ in 0..2 {
        b.press(&cx(&lib, queue, s), Key::Down);
    }
    assert_eq!(b.press(&cx(&lib, queue, s), Key::A), Outcome::ShuffleQueue);
    b.press(&cx(&lib, queue, s), Key::Down);
    b.press(&cx(&lib, queue, s), Key::A);
    assert_eq!(b.view(), View::Album(AlbumId(0)));
    let page = b.page(&cx(&lib, queue, s));
    assert_eq!(page.rows[page.selected], "two");
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::A),
        Outcome::PlayNext(ids(&[1]))
    );
    b.press(&cx(&lib, queue, s), Key::B);
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn a_two_line_list_s_first_page_holds_play_all_over_a_full_page_and_the_rest_a_page() {
    use crate::browse::RICH_ROWS;
    let files: Vec<String> = (0..12).map(|i| format!("X/A{i:02}/01 t.mp3")).collect();
    let (_d, lib) = library(&files.iter().map(String::as_str).collect::<Vec<_>>());
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    let page = |b: &Browser| b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let first = page(&b);
    assert_eq!(
        first.rows.len(),
        1 + RICH_ROWS,
        "Play All, and a page under it"
    );
    assert_eq!(first.pages, 3, "12 albums: 5, 5 and 2");
    // Down to the sixth album, past Play All.
    for _ in first.selected..=RICH_ROWS {
        press(&mut b, &lib, &[Key::Down]);
    }
    let next = page(&b);
    assert_eq!(
        (next.rows.len(), next.selected),
        (RICH_ROWS, 0),
        "a page, from its top"
    );
    assert_eq!(next.rows[0], "A05");
    press(&mut b, &lib, &[Key::Up]);
    assert_eq!(page(&b).selected, RICH_ROWS, "back on the first, its last");
    press(&mut b, &lib, &[Key::Left]);
    assert_eq!(
        page(&b).selected,
        0,
        "Left on the first page: its top, Play All"
    );
    press(&mut b, &lib, &[Key::Right]);
    let turned = page(&b);
    assert_eq!(
        turned.rows[turned.selected], "A05",
        "Right turns the page, from Play All too"
    );
}

#[test]
fn every_list_of_tracks_starts_with_play_all_and_shuffle() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "Y/B/01 two.mp3"]);
    for view in [
        View::Artists,
        View::Albums,
        View::Tracks,
        View::Artist(ArtistId(0)),
        View::Album(AlbumId(0)),
    ] {
        let rows = crate::rows(&Context::new(&lib, at(0)), view);
        assert_eq!(&rows[..1], &[crate::Row::PlayAll], "{view:?}");
    }
}

#[test]
fn shuffle_deals_what_the_list_shows() {
    let (_d, lib) = library(&["X/A/01 a1.mp3", "X/A/02 a2.mp3", "X/B/01 b1.mp3"]);
    let s = Settings::default();
    let mut b = Browser::default();
    // Albums, on Play All: held, it shuffles the albums, each kept whole.
    press(
        &mut b,
        &lib,
        &[Key::Down, Key::Down, Key::A, Key::Down, Key::A],
    );
    let cx = cx(&lib, QueueView::default(), s);
    assert_eq!(b.page(&cx).rows[0], "Play All");
    assert_eq!(b.mode(&cx, Key::A), crate::Mode::Hold);
    let out = b.held(&cx, Key::A);
    let Outcome::Play {
        tracks, shuffle, ..
    } = out
    else {
        panic!("{out:?}");
    };
    assert_eq!(shuffle, crate::Shuffle::Groups(vec![2, 1]));
    let mut p = Player::default();
    p.play(&lib, Queue::groups(tracks, &[2, 1], 5));
    let q = p.queue().tracks.to_vec();
    let a_together = q.windows(2).any(|w| *w == ids(&[0, 1])[..]);
    assert!(a_together, "album A plays whole and in order: {q:?}");
}

#[test]
fn the_first_row_resumes_when_paused_and_shuffles_everything_with_nothing_queued() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let s = Settings::default();
    let b = Browser::default();
    let row = |queue: QueueView| b.page(&cx(&lib, queue, s)).rows[0].clone();
    assert_eq!(row(QueueView::default()).text, "Shuffle Library");
    let mut first = Browser::default();
    assert_eq!(
        first.press(&cx(&lib, QueueView::default(), s), Key::A),
        Outcome::Play {
            tracks: ids(&[0]),
            start: 0,
            shuffle: crate::Shuffle::Tracks
        },
        "every song, shuffled"
    );
    assert_eq!(first.view(), View::NowPlaying);
    let tracks = ids(&[0]);
    let playing = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    assert_eq!(
        (row(playing).text.as_str(), row(playing).faint),
        ("Now Playing", false)
    );
    let paused = QueueView {
        playing: false,
        ..playing
    };
    assert_eq!(row(paused).text, "Resume");
}

#[test]
fn held_keys_move_and_seek_but_never_open_skip_or_wrap() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let cx = Context::new(&lib, at(0));
    let mut b = Browser::default();
    assert_eq!(b.mode(&cx, Key::Down), Mode::Repeat);
    assert_eq!(b.mode(&cx, Key::A), Mode::Once);
    assert_eq!(b.mode(&cx, Key::B), Mode::Hold, "B held goes home");
    assert!(b.at_end(&cx, Key::Up), "a held Up stops at the top");
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(
        b.mode(&cx, Key::L2),
        Mode::HoldRepeat,
        "seeking repeats held"
    );
    assert_eq!(
        b.mode(&cx, Key::R1),
        Mode::HoldRepeat,
        "skips tapped, seeks held"
    );
    assert_eq!(
        b.mode(&cx, Key::Right),
        Mode::HoldRepeat,
        "tapped skips, held seeks"
    );
    assert_eq!(b.held(&cx, Key::Right), Outcome::Seek(10));
}

#[test]
fn song_info_tells_the_track_from_its_file_and_start_closes_the_menu() {
    let (_d, lib) = library(&["X/A/03 Dream On.flac"]);
    let tracks = ids(&[0]);
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    let press = |b: &mut Browser, key| b.press(&cx(&lib, queue, s), key);
    press(&mut b, Key::Select);
    press(&mut b, Key::Start);
    for key in [Key::Up, Key::A] {
        press(&mut b, key);
    }
    assert_eq!(b.view(), View::SongInfo(TrackId(0)));
    let page = b.page(&cx(&lib, queue, s));
    let told: Vec<(&str, &str)> = page
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.after.as_str()))
        .collect();
    assert_eq!(
        told,
        [
            ("Title", "Dream On"),
            ("Artist", "X"),
            ("Album", "A"),
            ("Track", "03"),
            ("Format", "FLAC"),
            ("File", "03 Dream On.flac")
        ]
    );
    press(&mut b, Key::Start);
    assert_eq!(b.view(), View::PlayingMenu);
    press(&mut b, Key::Start);
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn notices_say_where_songs_went_as_players_say_it() {
    use crate::text::{notice, Notice};
    assert_eq!(notice(&Notice::Added(1)), "Added to Queue");
    assert_eq!(notice(&Notice::Added(3)), "Added 3 to Queue");
    assert_eq!(notice(&Notice::Next(1)), "Playing Next");
    assert_eq!(notice(&Notice::Next(12)), "Playing 12 Next");
}

#[test]
fn home_goes_all_the_way_back_to_where_the_cursor_was() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    press(&mut b, &lib, &[Key::A]);
    assert!(matches!(b.view(), View::Album(_)));
    b.home();
    assert_eq!(b.view(), View::Main);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rows[page.selected], "Library");
    // Library opens again on the row it was left on, as players keep their library's tab.
    press(&mut b, &lib, &[Key::A]);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rows[page.selected], "Albums");
}

#[test]
fn start_opens_a_menu_only_for_what_no_key_does() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let mut b = Browser::default();
    // An album plays, queues and opens from its keys: it has no menu.
    press(&mut b, &lib, &TO_ALBUMS);
    press(&mut b, &lib, &[Key::Start]);
    assert_eq!(b.view(), View::Albums);
    // A song's goes to its album or artist, or tells of it, for that song.
    press(&mut b, &lib, &[Key::A, Key::Down, Key::Down, Key::Start]);
    assert_eq!(b.view(), View::SongMenu(TrackId(1)));
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rows, ["Go to Album", "Go to Artist", "Song Info"]);
    press(&mut b, &lib, &[Key::Up, Key::A]);
    assert_eq!(b.view(), View::SongInfo(TrackId(1)));
    press(&mut b, &lib, &[Key::B, Key::Start]);
    assert_eq!(b.view(), View::Album(AlbumId(0)), "START closes it");
}

#[test]
fn list_hints_follow_the_row_and_its_state() {
    use crate::text::legend;
    use crate::{Legend, MainHold, Target};
    // Each hint as shown: its button and what it says.
    let said = |l: Legend| -> Vec<(&str, String)> {
        legend(l).iter().map(|h| (h.key, h.label())).collect()
    };
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.highlighted, Some(crate::Row::Album(AlbumId(0))));
    let song = said(Legend::List(Target::Song));
    assert_eq!(song[0], ("B", "Back".into()));
    assert_eq!(song[1], ("A", "Play (Next)".into()));
    assert_eq!(song[2], ("X", "Add to Queue".into()));
    assert_eq!(song[3], ("Y", "Favorite".into()), "the heart says which");
    let np = |playing| {
        said(Legend::NowPlaying {
            playing,
            lyrics: true,
        })
    };
    let none = said(Legend::NowPlaying {
        playing: true,
        lyrics: false,
    });
    assert_eq!(none[1], ("Y", "Favorite".into()), "no lyrics, no X");
    assert_eq!(np(false)[0], ("A", "Play".into()));
    assert_eq!(
        np(true)[..3],
        [
            ("A", "Pause".into()),
            ("X", "Lyrics".into()),
            ("Y", "Favorite".into())
        ]
    );
    let main = |hold| {
        said(Legend::Main {
            playing: false,
            hold,
        })
    };
    assert_eq!(
        main(Some(MainHold::Shuffle))[1],
        ("A", "Open (Shuffle)".into())
    );
    assert!(!legend(Legend::Main {
        playing: false,
        hold: None,
    })
    .iter()
    .any(|h| h.key == "SELECT"));
    let queued = said(Legend::Queue { restart: true });
    assert_eq!(queued[1], ("A", "Restart".into()));
    let queue = said(Legend::Queue { restart: false });
    assert_eq!(queue[2], ("X", "Remove".into()), "held, it goes on");
    assert_eq!(queue[3], ("Y", "Favorite (Move)".into()));
}

#[test]
fn x_adds_to_the_queue_and_y_favorites_a_song() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    assert_eq!(
        press(&mut b, &lib, &[Key::Y]),
        Outcome::None,
        "an album has no heart"
    );
    assert_eq!(
        press(&mut b, &lib, &[Key::X]),
        Outcome::Enqueue(ids(&[0, 1]))
    );
    press(&mut b, &lib, &[Key::A, Key::Down]);
    assert_eq!(
        press(&mut b, &lib, &[Key::Y]),
        Outcome::Favorite(TrackId(0))
    );
    assert_eq!(press(&mut b, &lib, &[Key::X]), Outcome::Enqueue(ids(&[0])));
}

#[test]
fn a_favorite_song_is_marked_in_every_list_it_shows_in() {
    let (d, _) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut lib = Library::scan(d.path());
    lib.toggle_favorite(TrackId(1));
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let marked: Vec<(&str, bool)> = page
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.favorite))
        .collect();
    assert_eq!(marked, [("Play All", false), ("one", false), ("two", true)]);
}

#[test]
fn a_held_on_the_music_menu_shuffles_a_list_or_plays_on_the_latest_book() {
    use crate::books::{Place, Shelf};
    let (_d, lib) = library(&[
        "X/A/01 one.mp3",
        "X/A/02 two.mp3",
        "../Audiobooks/Wilde - Tales/01 one.mp3",
    ]);
    let mut shelf = Shelf::new();
    let begun = Place {
        when: 1,
        ..Place::default()
    };
    shelf.set_place(&lib, BookId(0), begun);
    let cx = Context {
        shelf: &shelf,
        ..Context::new(&lib, at(0))
    };
    // Shuffle Library, Queue, then Library, which held shuffles every song.
    let mut b = Browser::default();
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    let Outcome::Play {
        tracks, shuffle, ..
    } = b.held(&cx, Key::A)
    else {
        panic!("Library held plays");
    };
    assert_eq!((tracks.len(), shuffle), (2, Shuffle::Tracks));
    assert_eq!(b.view(), View::NowPlaying);
    let mut b = Browser::default();
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    assert_eq!(
        b.held(&cx, Key::A),
        Outcome::PlayBook {
            book: BookId(0),
            over: false
        }
    );
    // In Audiobooks' lists, Y lists a book's bookmarks.
    let mut b = Browser::default();
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    b.press(&cx, Key::Y);
    assert_eq!(b.view(), View::Bookmarks(BookId(0)));
}

#[test]
fn x_on_the_music_menu_does_nothing_as_phones_have_no_stop() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let tracks = ids(&[0]);
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: false,
        waiting: false,
        speed: 1.0,
    };
    let cx = Context {
        queue,
        ..Context::new(&lib, at(0))
    };
    let mut b = Browser::default();
    assert_eq!(b.mode(&cx, Key::X), Mode::Once);
    assert_eq!(
        b.press(&cx, Key::X),
        Outcome::None,
        "pause is enough; the queue stays"
    );
}

#[test]
fn folders_open_from_the_library_play_all_under_them_and_a_song_plays_its_own_folder() {
    let (_d, lib) = library(&["Rock/A/01 a.mp3", "Rock/A/02 b.mp3", "Rock/c.mp3"]);
    let mut b = Browser::default();
    let page = |b: &Browser| b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let files = |outcome: Outcome| match outcome {
        Outcome::Play { tracks, start, .. } => {
            let name = |t: &TrackId| lib.tracks[*t].path.file_stem().unwrap().to_string_lossy();
            (tracks.iter().map(name).collect::<Vec<_>>(), start)
        }
        other => panic!("{other:?}"),
    };
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    let library = page(&b);
    let at = library.rows.iter().position(|r| *r == "Folders");
    let at = at.unwrap_or_else(|| panic!("Folders, in {}: {:?}", library.title, library.rows));
    for _ in library.selected..at {
        press(&mut b, &lib, &[Key::Down]);
    }
    press(&mut b, &lib, &[Key::A]);
    assert_eq!(page(&b).rows, ["Play All", "Rock"], "Music itself");
    press(&mut b, &lib, &[Key::Down, Key::A]);
    let rock = page(&b);
    assert_eq!(rock.title, "Rock");
    assert_eq!(
        rock.rows,
        ["Play All", "A", "c"],
        "its folders, then its songs by file"
    );
    assert_eq!(rock.selected, 0, "on Play All");
    let all = files(press(&mut b, &lib, &[Key::A]));
    assert_eq!(
        all,
        (vec!["01 a".into(), "02 b".into(), "c".into()], 0),
        "all under it"
    );
    press(&mut b, &lib, &[Key::B, Key::Down, Key::Down]);
    let own = files(press(&mut b, &lib, &[Key::A]));
    assert_eq!(
        own,
        (vec!["c".into()], 0),
        "a song: its own folder's songs, from it"
    );
}
