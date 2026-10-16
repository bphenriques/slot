use std::path::{Path, PathBuf};

use slot_music::{
    parse, text::Notice, track_title, Browser, Command, Context, Event, Key, Library, Mode,
    MuseState, Outcome, Player, Queue, QueueView, Settings, Shuffle, View,
};
use tempfile::TempDir;

fn library(files: &[&str]) -> (TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    for f in files {
        let path = d.path().join("Music").join(f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
    let lib = Library::scan(d.path());
    (d, lib)
}

/// What the browser reads: `lib` with `queue` and `settings`.
fn cx<'a>(lib: &'a Library, queue: QueueView<'a>, settings: Settings) -> Context<'a> {
    Context {
        queue,
        settings,
        ..Context::new(lib)
    }
}

fn names(lib: &Library) -> Vec<&str> {
    lib.artists.iter().map(|a| a.name.as_str()).collect()
}

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

fn press(b: &mut Browser, lib: &Library, keys: &[Key]) -> Outcome {
    keys.iter()
        .map(|&k| b.press(&cx(lib, QueueView::default(), Settings::default()), k))
        .last()
        .unwrap_or(Outcome::None)
}

// The main screen: Now Playing, Playlists, Artists, Albums, Songs, Settings. Artists and Albums
// open on Play All and Shuffle, so these go on to the first of them.
const TO_ARTISTS: [Key; 5] = [Key::Down, Key::Down, Key::A, Key::Down, Key::Down];
const TO_ALBUMS: [Key; 6] = [
    Key::Down,
    Key::Down,
    Key::Down,
    Key::A,
    Key::Down,
    Key::Down,
];
const TO_SETTINGS: [Key; 2] = [Key::Up, Key::A];
const TO_TRACKS: [Key; 5] = [Key::Down, Key::Down, Key::Down, Key::Down, Key::A];

#[test]
fn the_main_screen_keeps_its_rows_whether_or_not_anything_plays() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let b = Browser::default();
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(
        page.rows,
        [
            "Shuffle Library",
            "Playlists",
            "Artists",
            "Albums",
            "Songs",
            "Settings"
        ]
    );
    assert_eq!(page.pages, 1, "everything fits one page");
}

#[test]
fn rescan_reads_what_changed_and_rebuild_everything_last_in_settings() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    assert_eq!(
        press(&mut b, &lib, &[Key::Up, Key::A, Key::Up, Key::A]),
        Outcome::Tell(Notice::Hold("A")),
        "Rebuild acts only held"
    );
    let cx = Context::new(&lib);
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    assert_eq!(b.held(&cx, Key::A), Outcome::Rescan(true));
    assert_eq!(
        press(&mut b, &lib, &[Key::Up, Key::A]),
        Outcome::Rescan(false)
    );
    assert_eq!(b.view(), View::Settings);
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
    assert_eq!((start, shuffle), (0, slot_music::Shuffle::Off));
    assert_eq!(b.view(), View::NowPlaying);
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Artist(0));
}

#[test]
fn a_track_in_an_album_plays_from_there_to_the_end_of_the_album() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    // A -> (Play All, Shuffle, one, two) two
    let out = press(
        &mut b,
        &lib,
        &[Key::A, Key::Down, Key::Down, Key::Down, Key::A],
    );
    assert_eq!(
        out,
        Outcome::Play {
            tracks: lib.albums[0].tracks.clone(),
            start: 1,
            shuffle: slot_music::Shuffle::Off
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
    assert_eq!(at(&b), artists[Settings::default().list_size.page()]);
    press(&mut b, &lib, &[Key::Left]);
    assert_eq!(at(&b), "Abba");
}

#[test]
fn b_backs_out_one_level_and_leaves_from_the_top() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(b.view(), View::Artists);
    assert_eq!(press(&mut b, &lib, &[Key::B]), Outcome::None);
    assert_eq!(press(&mut b, &lib, &[Key::B]), Outcome::Leave);
}

fn played(c: Option<Command>) -> PathBuf {
    match c {
        Some(Command::Play { path, .. }) => path,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_player_moves_on_at_the_end_of_its_own_track_only() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    assert_eq!(
        played(p.play(&lib, Queue::new(vec![0, 1], 0, None))),
        lib.tracks[0].path
    );
    assert!(p.owns_audio());
    let stale = lib.tracks[1].path.clone();
    assert_eq!(p.on(&lib, Event::End(stale)), None);
    let next = p.on(&lib, Event::End(lib.tracks[0].path.clone()));
    assert_eq!(played(next), lib.tracks[1].path);
    assert_eq!(p.on(&lib, Event::End(lib.tracks[1].path.clone())), None);
    assert_eq!(p.state(), MuseState::Stopped);
    assert!(!p.owns_audio());
}

#[test]
fn a_track_muse_cannot_play_is_skipped_but_a_run_of_them_stops_the_queue() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new((0..5).collect(), 0, None));
    let error = || Event::Error("cannot open".into());
    assert_eq!(played(p.on(&lib, error())), lib.tracks[1].path);
    p.on(&lib, Event::Pos { at: 1.0, len: 9.0 });
    assert_eq!(played(p.on(&lib, error())), lib.tracks[2].path);
    assert_eq!(played(p.on(&lib, error())), lib.tracks[3].path);
    assert_eq!(p.on(&lib, error()), None);
    assert_eq!(p.state(), MuseState::Stopped);
}

#[test]
fn pause_resume_and_stop_hand_the_sound_back() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0], 0, None));
    assert_eq!(p.toggle(&lib), Some(Command::Pause));
    assert!(!p.owns_audio());
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
    assert!(p.owns_audio());
    assert_eq!(p.stop(), Some(Command::Stop));
    assert!(p.current().is_none() && !p.owns_audio());
    assert_eq!(p.pause(), None);
}

#[test]
fn shuffle_keeps_every_track_once() {
    let (_d, lib) = library(&["X/A/1 a.mp3", "X/A/2 b.mp3", "X/A/3 c.mp3", "X/A/4 d.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1, 2, 3], 0, Some(42)));
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(p.current().unwrap());
        p.skip(&lib, 1);
    }
    seen.sort();
    assert_eq!(seen, [0, 1, 2, 3]);
}

#[test]
fn commands_are_written_as_muse_lines() {
    let path = PathBuf::from("/m/A/x.mp3");
    assert_eq!(
        Command::Play {
            path,
            at: 0.0,
            speed: 1.0
        }
        .to_string(),
        "PLAY\tpath=/m/A/x.mp3"
    );
    assert_eq!(Command::Seek(12.0).to_string(), "SEEK\tat=12.0");
}

#[test]
fn muse_lines_parse_into_events() {
    assert_eq!(parse("READY\tproto=2"), None);
    assert!(matches!(parse("READY\tproto=3"), Some(Event::Error(_))));
    assert_eq!(
        parse("STATE\tstate=paused\tpath=/m/x.mp3\n"),
        Some(Event::State(MuseState::Paused))
    );
    assert_eq!(
        parse("POS\tat=12.5\tlen=200.0"),
        Some(Event::Pos {
            at: 12.5,
            len: 200.0
        })
    );
    assert_eq!(
        parse("END\tpath=/m/x.mp3"),
        Some(Event::End(Path::new("/m/x.mp3").into()))
    );
    assert_eq!(parse("META\ttitle=x\tchapters=2"), Some(Event::Meta));
    assert_eq!(
        parse("CHAPTER\ti=1\tat=61.5\ttitle=The Nightingale"),
        Some(Event::Chapter {
            at: 61.5,
            title: "The Nightingale".into()
        })
    );
    assert_eq!(parse("SINK\tdevice=default"), None);
}

#[test]
fn a_book_s_chapters_skip_within_it_and_its_speed_goes_to_muse_but_not_music_s() {
    let (_d, lib) = library(&[
        "../Audiobooks/Wilde - Tales/01 one.m4b",
        "../Audiobooks/Wilde - Tales/02 two.m4b",
        "X/A/01 song.mp3",
    ]);
    let song = lib.tracks_by_title[0];
    let mut p = Player::default();
    p.set_speed(1.5);
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    let play = p.play(&lib, Queue::new(parts, 0, None)).unwrap();
    assert!(play.to_string().ends_with("speed=1.50"), "{play}");
    p.on(&lib, Event::Meta);
    for (at, title) in [(0.0, "One"), (60.0, "Two"), (120.0, "Three")] {
        let title = title.into();
        p.on(&lib, Event::Chapter { at, title });
    }
    p.on(
        &lib,
        Event::Pos {
            at: 70.0,
            len: 180.0,
        },
    );
    assert_eq!(p.chapter(), Some((1, "Two", 60.0, 120.0)));
    assert_eq!(
        p.skip(&lib, 1),
        Some(Command::Seek(120.0)),
        "the next chapter"
    );
    p.on(
        &lib,
        Event::Pos {
            at: 120.5,
            len: 180.0,
        },
    );
    assert_eq!(
        p.skip(&lib, -1),
        Some(Command::Seek(60.0)),
        "just in: the one before"
    );
    p.on(
        &lib,
        Event::Pos {
            at: 150.0,
            len: 180.0,
        },
    );
    assert_eq!(
        p.skip(&lib, -1),
        Some(Command::Seek(120.0)),
        "well in: its start"
    );
    // Past the last chapter, the next file.
    assert!(matches!(p.skip(&lib, 1), Some(Command::Play { .. })));
    assert_eq!(p.chapter(), None, "a new file's chapters are its own");
    // Music plays as recorded whatever speed the book left.
    let music = p.play(&lib, Queue::new(vec![song], 0, None)).unwrap();
    assert!(!music.to_string().contains("speed"), "{music}");
}

#[test]
fn the_music_stays_audible_until_muse_confirms_it_stopped() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0], 0, None));
    p.on(&lib, Event::State(MuseState::Playing));
    p.stop();
    assert!(!p.owns_audio(), "a game may start opening the codec");
    assert!(p.audible(), "but Muse has not let go yet");
    p.on(&lib, Event::State(MuseState::Stopped));
    assert!(!p.audible());
}

#[test]
fn x_adds_what_a_row_stands_for_to_the_queue_and_a_held_plays_it_next() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(
        press(&mut b, &lib, &[Key::X]),
        Outcome::Enqueue(vec![0, 1, 2])
    );
    // X -> (Play All, Shuffle, A, B): album B
    press(&mut b, &lib, &[Key::A, Key::Down, Key::Down, Key::Down]);
    let out = press(&mut b, &lib, &[Key::X]);
    assert_eq!(out, Outcome::Enqueue(lib.albums[1].tracks.clone()));
    assert_eq!(b.view(), View::Artist(0), "adding is not opening");
    let cx = Context::new(&lib);
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    assert_eq!(
        b.held(&cx, Key::A),
        Outcome::PlayNext(lib.albums[1].tracks.clone())
    );
    assert_eq!(b.view(), View::Artist(0), "nor is putting next");
}

#[test]
fn the_queue_opens_on_what_plays_and_a_jumps_within_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1, 2], 1, None));
    let mut b = Browser::default();
    b.press(&cx(&lib, p.queue(), Settings::default()), Key::A);
    b.press(&cx(&lib, p.queue(), Settings::default()), Key::Select);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&cx(&lib, p.queue(), Settings::default()));
    let row = &page.rows[page.selected];
    assert_eq!(
        (row.text.as_str(), row.after.as_str(), row.playing),
        ("two", "X", true)
    );
    b.press(&cx(&lib, p.queue(), Settings::default()), Key::Down);
    assert_eq!(
        b.press(&cx(&lib, p.queue(), Settings::default()), Key::A),
        Outcome::Jump(2)
    );
    assert_eq!(played(p.jump(&lib, 2)), lib.tracks[2].path);
    assert_eq!(p.place(), (3, 3), "the queue itself is unchanged");
}

#[test]
fn pausing_for_a_game_keeps_the_queue_for_later() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1], 1, None));
    p.edit().enqueue(&[0]);
    assert_eq!(p.pause(), Some(Command::Pause));
    assert_eq!((p.current(), p.place()), (Some(1), (2, 3)));
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
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
fn repeat_one_replays_and_repeat_all_wraps_round() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1], 1, None));
    p.set_repeat(slot_music::Repeat::One);
    let again = p.on(&lib, Event::End(lib.tracks[1].path.clone()));
    assert_eq!(played(again), lib.tracks[1].path);
    p.set_repeat(slot_music::Repeat::All);
    let wrapped = p.on(&lib, Event::End(lib.tracks[1].path.clone()));
    assert_eq!(played(wrapped), lib.tracks[0].path);
    assert_eq!(played(p.skip(&lib, -1)), lib.tracks[1].path);
}

#[test]
fn music_settings_flip_with_left_and_right() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_SETTINGS);
    assert_eq!(b.view(), View::Settings);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(
        page.rows,
        [
            "Screen Off",
            "Jump Length",
            "List Size",
            "Dynamic Color",
            "Theme",
            "Update Library",
            "Rebuild Library"
        ]
    );
    assert_eq!(
        page.values,
        [
            Some("Pause".to_string()),
            Some("30 s".to_string()),
            Some("Normal".to_string()),
            Some("On".to_string()),
            Some("Slot".to_string()),
            None,
            None
        ]
    );
    assert_eq!(
        press(&mut b, &lib, &[Key::Right]),
        Outcome::Change(slot_music::Setting::ScreenOff, 1)
    );
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Main);
}

#[test]
fn settings_are_kept_on_the_card_but_the_sleep_timer_is_not() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(Settings::read(d.path()), Settings::default());
    let s = Settings {
        repeat: slot_music::Repeat::One,
        clock: true,
        up_next: false,
        color: false,
        jump: 15,
        ..Settings::default()
    };
    Settings { ..s }.write(d.path());
    assert_eq!(Settings::read(d.path()), s);
    assert_eq!(slot_music::Repeat::One.next(), slot_music::Repeat::Off);
}

#[test]
fn the_queue_comes_back_from_the_card_paused_where_it_was() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let text = slot_music::queue_file::encode(d.path(), &lib, &[2, 0, 1], 1, 42.5);
    assert!(text.lines().any(|l| l == "Music/X/A/01 one.mp3"));
    let (queue, at, pos) = slot_music::queue_file::decode(&text, d.path(), &lib);
    assert_eq!((queue, at, pos), (vec![2, 0, 1], 1, 42.5));
    let mut p = Player::default();
    p.restore(vec![2, 0, 1], 1, 42.5, false);
    assert!(p.resumable() && !p.owns_audio());
    assert_eq!(
        p.toggle(&lib),
        Some(Command::Play {
            path: lib.tracks[0].path.clone(),
            at: 42.5,
            speed: 1.0
        })
    );
}

#[test]
fn a_queued_track_gone_from_the_library_is_dropped_and_its_place_moves_on() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/03 three.mp3"]);
    let text = "#EXTM3U\n#SLOT-AT:1\n#SLOT-POS:30.0\nMusic/X/A/01 one.mp3\nMusic/X/A/02 gone.mp3\nMusic/X/A/03 three.mp3\n";
    let (queue, at, pos) = slot_music::queue_file::decode(text, d.path(), &lib);
    assert_eq!((queue, at, pos), (vec![0, 1], 1, 0.0));
}

#[test]
fn the_queue_opens_on_the_playing_track_and_x_held_goes_on_taking_tracks_out() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = [0, 1];
    let queue = QueueView {
        tracks: &tracks,
        at: 1,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, Settings::default()), Key::A);
    b.press(&cx(&lib, queue, Settings::default()), Key::Select);
    let page = b.page(&cx(&lib, queue, Settings::default()));
    assert_eq!(page.rows[0], "one");
    assert_eq!(page.selected, 1, "the playing track");
    let s = Settings::default();
    assert_eq!(b.mode(&cx(&lib, queue, s), Key::X), Mode::Repeat);
    assert_eq!(b.press(&cx(&lib, queue, s), Key::X), Outcome::Remove(1));
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::X),
        Outcome::None,
        "it stops at the last"
    );
    b.press(&cx(&lib, queue, s), Key::Up);
    assert_eq!(b.held(&cx(&lib, queue, s), Key::X), Outcome::Remove(0));
    b.press(&cx(&lib, queue, s), Key::Start);
    assert_eq!(b.view(), View::Queue, "no menu: the keys have it all");
}

#[test]
fn tracks_play_or_shuffle_everything_from_their_top_rows() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    assert_eq!(b.view(), View::Tracks);
    let out = press(&mut b, &lib, &[Key::Down, Key::A]);
    assert_eq!(
        out,
        Outcome::Play {
            tracks: lib.tracks_by_title.clone(),
            start: 0,
            shuffle: slot_music::Shuffle::Tracks
        }
    );
}

#[test]
fn repeat_all_goes_round_the_same_shuffle() {
    let files: Vec<String> = (1..=8).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.set_repeat(slot_music::Repeat::All);
    p.play(&lib, Queue::new((0..8).collect(), 0, Some(7)));
    let first: Vec<usize> = p.queue().tracks.to_vec();
    assert_eq!(p.up_next(), Some(first[1]));
    p.skip(&lib, 7);
    assert_eq!(p.up_next(), Some(first[0]), "round to the start");
    p.skip(&lib, 1);
    assert_eq!(p.queue().tracks, first.as_slice());
    assert_eq!(p.current(), Some(first[0]));
    p.edit().toggle_shuffle(1);
    assert_eq!(
        p.queue().tracks,
        (0..8).collect::<Vec<_>>(),
        "back in order"
    );
}

#[test]
fn the_player_says_when_the_queue_is_worth_keeping() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    assert!(!p.take_changed());
    p.play(&lib, Queue::new(vec![0], 0, None));
    assert!(p.take_changed() && !p.take_changed(), "once per change");
    p.on(&lib, Event::Pos { at: 3.0, len: 9.0 });
    assert!(!p.take_changed(), "the clock ticking is not one");
    p.pause();
    assert!(p.take_changed());
}

#[test]
fn now_playing_skips_with_l1_r1_and_seeks_with_l2_r2() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(press(&mut b, &lib, &[Key::X]), Outcome::ShuffleQueue);
    let cx = Context::new(&lib);
    assert_eq!(b.mode(&cx, Key::X), Mode::Once, "repeat is on START's menu");
    assert_eq!(press(&mut b, &lib, &[Key::L1]), Outcome::Skip(-1));
    assert_eq!(press(&mut b, &lib, &[Key::R1]), Outcome::Skip(1));
    assert_eq!(press(&mut b, &lib, &[Key::L2]), Outcome::Seek(-10));
    assert_eq!(press(&mut b, &lib, &[Key::R2]), Outcome::Seek(10));
    assert_eq!(
        press(&mut b, &lib, &[Key::Right]),
        Outcome::Skip(1),
        "as R1 does"
    );
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
fn shuffling_the_queue_keeps_the_track_playing_and_undoes_to_the_old_order() {
    let files: Vec<String> = (1..=6).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new((0..6).collect(), 3, None));
    p.edit().toggle_shuffle(9);
    assert!(p.shuffled());
    assert_eq!(p.current(), Some(3));
    assert_eq!(p.queue().tracks[0], 3, "what plays comes first");
    p.edit().toggle_shuffle(9);
    assert!(!p.shuffled());
    assert_eq!(p.queue().tracks, &[0, 1, 2, 3, 4, 5]);
    assert_eq!(p.current(), Some(3));
}

#[test]
fn lists_count_their_items_and_not_the_rows_above_them() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let page = |b: &Browser| b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!((page(&b).item, page(&b).items), (None, 3), "on Play All");
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::Down]);
    assert_eq!((page(&b).item, page(&b).items), (Some(2), 3));
}

#[test]
fn a_track_keeps_the_number_its_file_starts_with() {
    assert_eq!(slot_music::track_number("03 Dream On.mp3"), "03");
    assert_eq!(slot_music::track_number("1-03 - Dream On.flac"), "1-03");
    assert_eq!(slot_music::track_number("Dream On.mp3"), "");
    assert_eq!(slot_music::track_number("1999.mp3"), "");
}

#[test]
fn rows_name_their_context_dim_and_mark_what_plays() {
    let (_d, lib) = library(&[
        "Aerosmith/Get a Grip/03 Cryin.mp3",
        "Blur/Parklife/01 Girls.mp3",
    ]);
    let settings = Settings::default();
    let current = [0];
    let queue = QueueView {
        tracks: &current,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let mut b = Browser::default();
    // The albums of everyone name whose they are.
    press(&mut b, &lib, &TO_ALBUMS);
    let page = b.page(&cx(&lib, queue, settings));
    assert_eq!(
        (page.rows[2].text.as_str(), page.rows[2].after.as_str()),
        ("Get a Grip", "Aerosmith")
    );
    // An album's own tracks go by their numbers, and what plays is marked.
    b.press(&cx(&lib, queue, settings), Key::A);
    let page = b.page(&cx(&lib, queue, settings));
    let row = &page.rows[2];
    assert_eq!((row.before.as_str(), row.text.as_str()), ("", "Cryin"));
    assert!(row.playing && row.after.is_empty());
    assert_eq!(
        row.icon,
        Some(slot_music::RowIcon::Play),
        "the play symbol stands in for 03"
    );
    assert!(page.rows[0].action && !row.action);
    assert_eq!(page.rows[0].icon, None, "action rows go without icons");
}

#[test]
fn the_main_screen_counts_what_its_rows_lead_to() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/B/01 two.mp3"]);
    let page = Browser::default().page(&cx(&lib, QueueView::default(), Settings::default()));
    let values: Vec<Option<&str>> = page.values.iter().map(Option::as_deref).collect();
    assert_eq!(
        values,
        [None, None, Some("1"), Some("2"), Some("2"), None],
        "Now Playing, Playlists, Artists, Albums, Songs, Settings"
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
fn the_queue_opens_below_now_playing_and_a_pick_goes_back_to_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = [0, 1];
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, s), Key::Select);
    b.press(&cx(&lib, queue, s), Key::Select);
    assert_eq!(b.view(), View::Queue);
    b.press(&cx(&lib, queue, s), Key::Down);
    assert_eq!(b.press(&cx(&lib, queue, s), Key::A), Outcome::Jump(1));
    assert_eq!(b.view(), View::NowPlaying);
    b.press(&cx(&lib, queue, s), Key::B);
    assert_eq!(b.view(), View::Main, "no pile of screens behind it");
}

#[test]
fn the_menu_goes_to_the_album_on_the_track_and_a_held_in_a_list_plays_next() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let tracks = [1];
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
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
            "Add to Favorites",
            "Sleep Timer",
            "Repeat",
            "Go to",
            "Go to",
            "Song Info"
        ],
        "what to do first, then where to go"
    );
    assert_eq!(page.rows[3].after, "A", "the name set dim after");
    assert_eq!(b.press(&cx(&lib, queue, s), Key::A), Outcome::Favorite(1));
    for _ in 0..3 {
        b.press(&cx(&lib, queue, s), Key::Down);
    }
    b.press(&cx(&lib, queue, s), Key::A);
    assert_eq!(b.view(), View::Album(0));
    let page = b.page(&cx(&lib, queue, s));
    assert_eq!(page.rows[page.selected], "two");
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::A),
        Outcome::PlayNext(vec![1])
    );
    b.press(&cx(&lib, queue, s), Key::B);
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn play_next_goes_right_after_what_plays_and_survives_unshuffling() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1, 2], 0, None));
    p.edit().play_next(&[4]);
    assert_eq!(p.queue().tracks, &[0, 4, 1, 2]);
    p.edit().toggle_shuffle(3);
    p.edit().play_next(&[3]);
    p.edit().toggle_shuffle(3);
    assert_eq!(p.queue().tracks, &[0, 3, 4, 1, 2]);
}

#[test]
fn every_list_of_tracks_starts_with_play_all_and_shuffle() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "Y/B/01 two.mp3"]);
    for view in [
        View::Artists,
        View::Albums,
        View::Tracks,
        View::Artist(0),
        View::Album(0),
    ] {
        let rows = slot_music::rows(&Context::new(&lib), view);
        assert_eq!(
            &rows[..2],
            &[slot_music::Row::PlayAll, slot_music::Row::Shuffle],
            "{view:?}"
        );
    }
}

#[test]
fn shuffle_deals_what_the_list_shows() {
    let (_d, lib) = library(&["X/A/01 a1.mp3", "X/A/02 a2.mp3", "X/B/01 b1.mp3"]);
    let s = Settings::default();
    let mut b = Browser::default();
    // Albums: Play All, Shuffle Albums, ...
    press(
        &mut b,
        &lib,
        &[Key::Down, Key::Down, Key::Down, Key::A, Key::Down],
    );
    assert_eq!(
        b.page(&cx(&lib, QueueView::default(), s)).rows[1],
        "Shuffle Albums"
    );
    let out = press(&mut b, &lib, &[Key::A]);
    let Outcome::Play {
        tracks, shuffle, ..
    } = out
    else {
        panic!("{out:?}");
    };
    assert_eq!(shuffle, slot_music::Shuffle::Groups(vec![2, 1]));
    let mut p = Player::default();
    p.play(&lib, Queue::groups(tracks, &[2, 1], 5));
    let q = p.queue().tracks.to_vec();
    let a_together = q.windows(2).any(|w| w == [0, 1]);
    assert!(a_together, "album A plays whole and in order: {q:?}");
}

#[test]
fn the_queue_can_lose_and_move_tracks_around_what_plays() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1, 2, 3, 4], 2, None));
    p.edit().remove(2);
    assert_eq!(p.queue().tracks, &[0, 1, 2, 3, 4], "not the one playing");
    p.edit().remove(0);
    assert_eq!(
        (p.queue().tracks, p.current()),
        (&[1, 2, 3, 4][..], Some(2))
    );
    p.edit().move_to(3, 2);
    assert_eq!(p.queue().tracks, &[1, 2, 4, 3]);
    assert_eq!(p.current(), Some(2));
}

#[test]
fn in_the_queue_y_lifts_a_track_to_carry_and_held_favorites_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let tracks = [0, 1, 2];
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    let press = |b: &mut Browser, key| b.press(&cx(&lib, queue, s), key);
    press(&mut b, Key::Select);
    press(&mut b, Key::Select);
    assert_eq!(
        press(&mut b, Key::R1),
        Outcome::None,
        "the shoulders turn pages"
    );
    assert_eq!(press(&mut b, Key::L1), Outcome::None);
    press(&mut b, Key::Down);
    assert_eq!(b.held(&cx(&lib, queue, s), Key::Y), Outcome::Favorite(1));
    assert_eq!(press(&mut b, Key::Y), Outcome::None, "lifted");
    assert_eq!(press(&mut b, Key::Down), Outcome::Move(1, 2));
    assert_eq!(press(&mut b, Key::Down), Outcome::None, "the end");
    assert_eq!(
        press(&mut b, Key::X),
        Outcome::None,
        "keys wait while carrying"
    );
    assert_eq!(
        press(&mut b, Key::B),
        Outcome::Move(2, 1),
        "back where it was"
    );
    assert_eq!(
        press(&mut b, Key::B),
        Outcome::None,
        "put down: B goes back"
    );
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn a_carried_track_can_be_put_next_and_a_played_one_shows_faint() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let tracks = [0, 1, 2];
    let queue = QueueView {
        tracks: &tracks,
        at: 1,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, s), Key::Select);
    b.press(&cx(&lib, queue, s), Key::Select);
    let page = b.page(&cx(&lib, queue, s));
    assert!(page.rows[0].faint && !page.rows[1].faint);
    // The last track, lifted and put next: just after the one playing.
    b.press(&cx(&lib, queue, s), Key::Down);
    b.press(&cx(&lib, queue, s), Key::Y);
    assert_eq!(b.press(&cx(&lib, queue, s), Key::R1), Outcome::Move(2, 2));
    assert_eq!(b.press(&cx(&lib, queue, s), Key::X), Outcome::Remove(2));
    // A held puts it next at once; tapped, it plays.
    assert_eq!(b.mode(&cx(&lib, queue, s), Key::A), Mode::Hold);
    assert_eq!(b.held(&cx(&lib, queue, s), Key::A), Outcome::Move(2, 2));
    b.press(&cx(&lib, queue, s), Key::Up);
    assert_eq!(
        b.mode(&cx(&lib, queue, s), Key::A),
        Mode::Once,
        "it restarts at once"
    );
}

#[test]
fn moving_a_track_keeps_the_playing_one_playing() {
    let mut q = Queue::new(vec![10, 11, 12, 13], 1, None);
    q.move_to(3, 0);
    assert_eq!((q.tracks(), q.current()), (&[13, 10, 11, 12][..], Some(11)));
    q.move_to(2, 3);
    assert_eq!((q.tracks(), q.current()), (&[13, 10, 12, 11][..], Some(11)));
}

#[test]
fn the_list_size_sets_how_many_rows_a_page_holds() {
    use slot_music::ListSize;
    assert_eq!(
        [ListSize::Small, ListSize::Normal, ListSize::Large].map(ListSize::page),
        [10, 8, 7]
    );
    let mut settings = Settings::default();
    settings.change(slot_music::Setting::ListSize, 1);
    assert_eq!(settings.list_size, ListSize::Large);
    settings.change(slot_music::Setting::ListSize, 1);
    assert_eq!(settings.list_size, ListSize::Large, "it stops at the ends");
    let d = tempfile::tempdir().unwrap();
    let s = Settings {
        list_size: ListSize::Small,
        ..Settings::default()
    };
    s.write(d.path());
    assert_eq!(Settings::read(d.path()).list_size, ListSize::Small);
}

#[test]
fn after_a_rescan_the_track_playing_plays_on_without_asking_muse_again() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1], 1, None));
    p.forget();
    assert!(
        p.current().is_none() && p.owns_audio(),
        "Muse keeps the codec meanwhile"
    );
    p.restore(vec![0, 1], 1, 30.0, true);
    assert_eq!(p.current(), Some(1));
    assert!(!p.resumable() && p.owns_audio());
    assert_eq!(
        p.toggle(&lib),
        Some(Command::Pause),
        "it is playing, not waiting to"
    );
}

#[test]
fn a_goes_round_the_choices_while_left_and_right_stop_at_the_ends() {
    use slot_music::cycle;
    let order = [1, 2, 3];
    assert_eq!(cycle(&order, 3, 0), 1);
    assert_eq!(cycle(&order, 1, -1), 1);
    assert_eq!(cycle(&order, 1, 1), 2);
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
            tracks: vec![0],
            start: 0,
            shuffle: slot_music::Shuffle::Tracks
        },
        "every song, shuffled"
    );
    assert_eq!(first.view(), View::NowPlaying);
    let tracks = [0];
    let playing = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
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
    let cx = Context::new(&lib);
    let mut b = Browser::default();
    assert_eq!(b.mode(&cx, Key::Down), Mode::Repeat);
    assert_eq!(b.mode(&cx, Key::A), Mode::Once);
    assert_eq!(b.mode(&cx, Key::B), Mode::Hold, "B held goes home");
    assert!(b.at_end(&cx, Key::Up), "a held Up stops at the top");
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(b.mode(&cx, Key::L2), Mode::Repeat, "seeking repeats");
    assert_eq!(b.mode(&cx, Key::R1), Mode::Once, "skipping does not");
    assert_eq!(
        b.mode(&cx, Key::Right),
        Mode::HoldRepeat,
        "tapped skips, held seeks"
    );
    assert_eq!(b.held(&cx, Key::Right), Outcome::Seek(10));
}

#[test]
fn up_next_is_the_next_in_the_queue_or_the_first_again_under_repeat_all() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1], 0, None));
    assert_eq!(p.up_next(), Some(1));
    p.skip(&lib, 1);
    assert_eq!(p.up_next(), None, "the end");
    p.set_repeat(slot_music::Repeat::All);
    assert_eq!(p.up_next(), Some(0));
}

#[test]
fn the_queue_shows_the_whole_shuffled_queue_after_skips() {
    let files: Vec<String> = (1..=60).map(|i| format!("X/A/{i:02} t{i}.mp3")).collect();
    let (_d, lib) = library(&files.iter().map(String::as_str).collect::<Vec<_>>());
    let mut p = Player::default();
    p.set_repeat(slot_music::Repeat::One);
    p.play(&lib, Queue::new((0..60).collect(), 0, Some(7)));
    for _ in 0..36 {
        p.skip(&lib, 1);
    }
    let mut b = Browser::default();
    let s = Settings::default();
    b.press(&cx(&lib, p.queue(), s), Key::Select);
    b.press(&cx(&lib, p.queue(), s), Key::Select);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&cx(&lib, p.queue(), s));
    assert_eq!(page.rows.len(), s.list_size.page(), "{:?}", page.rows);
    assert_eq!(page.selected, 36 % s.list_size.page(), "36 played");
    assert!(page.rows.iter().all(|r| !r.text.is_empty()));
}

#[test]
fn song_info_tells_the_track_from_its_file_and_start_closes_the_menu() {
    let (_d, lib) = library(&["X/A/03 Dream On.flac"]);
    let tracks = [0];
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &[],
        chapter: None,
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
    assert_eq!(b.view(), View::SongInfo(0));
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
fn back_starts_the_track_over_once_it_has_played_a_second() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0, 1], 1, None));
    p.on(
        &lib,
        Event::Pos {
            at: 10.0,
            len: 60.0,
        },
    );
    assert_eq!(p.skip(&lib, -1), Some(Command::Seek(0.0)), "the start");
    assert_eq!(p.current(), Some(1));
    p.on(&lib, Event::Pos { at: 0.5, len: 60.0 });
    assert_eq!(
        played(p.skip(&lib, -1)),
        lib.tracks[0].path,
        "the one before"
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
    let read: Vec<(&str, &[usize])> = lib
        .playlists
        .iter()
        .map(|p| (p.name.as_str(), p.tracks.as_slice()))
        .collect();
    assert_eq!(
        read,
        [("Favorites", &[][..]), ("Road Trip", &[2, 0, 1][..])]
    );
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::A]);
    assert_eq!(b.view(), View::Playlists);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rows, ["Favorites", "Recently Added", "Road Trip"]);
    assert_eq!(page.values[2].as_deref(), Some("3"), "its songs counted");
}

#[test]
fn favorites_is_written_as_a_playlist_other_players_read() {
    let (d, _) = library(&["X/A/01 one.mp3"]);
    let mut lib = Library::scan(d.path());
    lib.playlists[0].tracks.push(0);
    slot_music::write_playlist(&lib.playlists[0], &lib);
    let text = std::fs::read_to_string(d.path().join("Music/Playlists/Favorites.m3u8")).unwrap();
    assert_eq!(text, "#EXTM3U\n../X/A/01 one.mp3\n");
    assert_eq!(Library::scan(d.path()).playlists[0].tracks, [0]);
}

#[test]
fn a_saved_queue_reads_resume_before_the_library_is_read() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.wait_for_saved();
    let b = Browser::default();
    let page = b.page(&cx(&lib, p.queue(), Settings::default()));
    assert_eq!(page.rows[0], "Resume", "not Now Playing, then Resume");
    p.restore(vec![0], 0, 0.0, false);
    let page = b.page(&cx(&lib, p.queue(), Settings::default()));
    assert_eq!(page.rows[0], "Resume");
}

fn record(path: &str, tags: slot_music::Tags) -> slot_music::Record {
    slot_music::Record {
        path: PathBuf::from("/card/Music").join(path),
        modified: 0,
        added: 0,
        size: 0,
        tags,
    }
}

#[test]
fn tags_come_first_and_folders_fill_in_what_they_leave_out() {
    use slot_music::Tags;
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
                    ..Tags::default()
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
                    ..Tags::default()
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
                    ..Tags::default()
                },
            ),
            record(
                "X/Big/b.mp3",
                Tags {
                    title: "One".into(),
                    album: "Big".into(),
                    disc: 1,
                    track: 5,
                    ..Tags::default()
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
    let progress = slot_music::Progress::default();
    slot_music::scan_library(d.path(), false, &progress);
    let cache = d.path().join(slot_music::CACHE_FILE);
    // Tags only the cache knows: an unchanged song keeps them, a full scan does not.
    let text = std::fs::read_to_string(&cache).unwrap();
    std::fs::write(&cache, text.replace("\t\t\t\t\t", "\tCached\t\t\t\t")).unwrap();
    // The cache's library comes at once, without the card; the scan then finds it the same.
    let shown = slot_music::cached_library(d.path()).unwrap();
    assert_eq!(shown.tracks[0].title, "Cached");
    let (lib, changed) = slot_music::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("Cached", false));
    let (lib, changed) = slot_music::scan_library(d.path(), true, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("one", true));
    // A changed song is read again.
    std::fs::write(&cache, text.replace("\t\t\t\t\t", "\tCached\t\t\t\t")).unwrap();
    std::fs::write(d.path().join("Music/X/A/01 one.mp3"), b"changed").unwrap();
    let (lib, changed) = slot_music::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks[0].title.as_str(), changed), ("one", true));
    // A song gone is a change too.
    std::fs::remove_file(d.path().join("Music/X/A/01 one.mp3")).unwrap();
    let (lib, changed) = slot_music::scan_library(d.path(), false, &progress);
    assert_eq!((lib.tracks.len(), changed), (0, true));
}

#[test]
fn recently_added_is_by_when_the_library_got_them_and_an_album_artist_names_the_song() {
    use slot_music::Tags;
    let added = |path: &str, added: u64, modified: u64, tags: Tags| slot_music::Record {
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
                    ..Tags::default()
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
fn notices_say_where_songs_went_in_one_short_pattern() {
    use slot_music::text::{notice, Notice};
    assert_eq!(notice(&Notice::Added(1)), "Up Next +1");
    assert_eq!(notice(&Notice::Next(12)), "Play Next +12");
}

#[test]
fn a_book_says_its_speed_how_far_through_it_is_and_its_time_left() {
    use slot_music::text::book_left;
    assert_eq!(
        book_left(1.25, Some(0.374), 3723.0),
        "1.25× · 37% · 1:02:03 left"
    );
    assert_eq!(
        book_left(1.0, None, 90.0),
        "1× · 1:30 left",
        "parts of unknown length"
    );
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
    press(
        &mut b,
        &lib,
        &[Key::A, Key::Down, Key::Down, Key::Down, Key::Start],
    );
    assert_eq!(b.view(), View::RowMenu(slot_music::Row::Track(1)));
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.rows, ["Go to", "Go to", "Song Info"]);
    press(&mut b, &lib, &[Key::Up, Key::A]);
    assert_eq!(b.view(), View::SongInfo(1));
    press(&mut b, &lib, &[Key::B, Key::Start]);
    assert_eq!(b.view(), View::Album(0), "START closes it");
}

#[test]
fn list_hints_follow_the_row_and_its_state() {
    use slot_music::text::legend;
    use slot_music::{Legend, MainHold, Target};
    // Each hint as shown: its button and what it says.
    let said = |l: Legend| -> Vec<(&str, String)> {
        legend(l).iter().map(|h| (h.key, h.label())).collect()
    };
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ALBUMS);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    assert_eq!(page.highlighted, Some(slot_music::Row::Album(0)));
    let song = said(Legend::List(Target::Song));
    assert_eq!(song[0], ("A", "Play (Next)".into()));
    assert_eq!(song[1], ("X", "Add to Queue".into()));
    assert_eq!(song[2], ("Y", "Favorite".into()), "the heart says which");
    let np = |playing| said(Legend::NowPlaying { playing });
    assert_eq!(np(false)[0], ("A", "Play".into()));
    assert_eq!(
        np(true)[..3],
        [
            ("A", "Pause".into()),
            ("X", "Shuffle".into()),
            ("Y", "Favorite".into())
        ]
    );
    let main = |hold| {
        said(Legend::Main {
            playing: false,
            hold,
            stop: false,
        })
    };
    assert_eq!(
        main(Some(MainHold::Shuffle))[1],
        ("A", "Open (Shuffle)".into())
    );
    assert!(!legend(Legend::Main {
        playing: false,
        hold: None,
        stop: false,
    })
    .iter()
    .any(|h| h.key == "SELECT"));
    let queued = said(Legend::Queue { restart: true });
    assert_eq!(queued[0], ("A", "Restart".into()));
    let queue = said(Legend::Queue { restart: false });
    assert_eq!(queue[1], ("X", "Remove".into()), "held, it goes on");
    assert_eq!(queue[2], ("Y", "Move (Favorite)".into()));
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
    assert_eq!(press(&mut b, &lib, &[Key::X]), Outcome::Enqueue(vec![0, 1]));
    press(&mut b, &lib, &[Key::A, Key::Down, Key::Down]);
    assert_eq!(press(&mut b, &lib, &[Key::Y]), Outcome::Favorite(0));
    assert_eq!(press(&mut b, &lib, &[Key::X]), Outcome::Enqueue(vec![0]));
}

#[test]
fn an_artist_lists_albums_by_year_and_discs_are_ruled_apart() {
    use slot_music::Tags;
    let tagged = |path: &str, album: &str, year: u32, disc: u32, track: u32| {
        record(
            path,
            Tags {
                artist: "X".into(),
                album: album.into(),
                year,
                disc,
                track,
                ..Tags::default()
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
    assert_eq!(
        page.rules,
        [2, 3],
        "under Play All and Shuffle, then disc 2"
    );
}

#[test]
fn resume_before_the_saved_queue_is_back_plays_once_it_is() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.wait_for_saved();
    let mut b = Browser::default();
    assert_eq!(
        b.press(&cx(&lib, p.queue(), Settings::default()), Key::A),
        Outcome::Toggle,
        "Resume plays, not only opens"
    );
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn a_favorite_song_is_marked_in_every_list_it_shows_in() {
    let (d, _) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut lib = Library::scan(d.path());
    lib.toggle_favorite(1);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let marked: Vec<(&str, bool)> = page
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.favorite))
        .collect();
    assert_eq!(
        marked,
        [
            ("Play All", false),
            ("Shuffle Songs", false),
            ("one", false),
            ("two", true)
        ]
    );
}

#[test]
fn audiobooks_are_kept_apart_from_the_music_and_open_from_the_menu() {
    let (_d, lib) = library(&[
        "X/A/01 one.mp3",
        "../Audiobooks/Kipling - Just So/01 whale.mp3",
        "../Audiobooks/Kipling - Just So/02 camel.mp3",
        "../Audiobooks/Wilde - The Happy Prince.m4b",
    ]);
    assert_eq!((lib.music_artists.len(), lib.music_tracks().len()), (1, 1));
    assert_eq!(lib.albums_by_name.len(), 1, "books are not albums");
    let mut books: Vec<(&str, &str, usize)> = lib
        .books
        .iter()
        .map(|b| {
            let book = &lib.albums[b.album];
            let author = lib.artists[book.artist].name.as_str();
            (book.name.as_str(), author, book.tracks.len())
        })
        .collect();
    books.sort();
    assert_eq!(
        books,
        [("Just So", "Kipling", 2), ("The Happy Prince", "Wilde", 1)]
    );
    let mut b = Browser::default();
    // Shuffle Library, Playlists, then Audiobooks.
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    assert_eq!(b.view(), View::Books);
    // The first by title, as none is begun.
    let book = 0;
    assert_eq!(
        press(&mut b, &lib, &[Key::A]),
        Outcome::PlayBook { book, over: false }
    );
    assert_eq!(b.view(), View::NowPlaying);
}

/// A library of two books, the first of two parts a minute and three minutes long.
fn books() -> (TempDir, Library) {
    let (d, mut lib) = library(&[
        "../Audiobooks/Kipling - Just So/01 whale.mp3",
        "../Audiobooks/Kipling - Just So/02 camel.mp3",
        "../Audiobooks/Wilde - The Happy Prince.m4b",
    ]);
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    for (&t, secs) in parts.iter().zip([60.0, 180.0]) {
        lib.tracks[t].secs = secs;
    }
    lib.books[0].secs = 240.0;
    (d, lib)
}

#[test]
fn books_list_the_one_listened_to_first_with_how_far_by_the_parts_lengths() {
    use slot_music::books::{Place, Shelf};
    let (_d, lib) = books();
    let mut shelf = Shelf::new();
    let place = Place {
        part: 1,
        pos: 60.0,
        when: 1_700_000_000,
        ..Place::default()
    };
    shelf.set_place(&lib, 1, Place { when: 1, ..place });
    shelf.set_place(&lib, 0, place);
    let cx = Context {
        shelf: &shelf,
        ..Context::new(&lib)
    };
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    let page = b.page(&cx);
    assert_eq!(page.rows[0].text, "Just So", "the latest listened to");
    // A minute into the second part, after a minute-long first: two of four minutes.
    assert_eq!(page.values[0].as_deref(), Some("50%"));
    assert_eq!(page.rows[1].text, "The Happy Prince");
}

#[test]
fn a_book_s_keys_jump_mark_and_change_speed_and_list_its_marks_and_chapters() {
    let (_d, lib) = books();
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    let chapters = [(0.0, "Whale".to_string()), (30.0, "Throat".to_string())];
    let queue = QueueView {
        tracks: &parts,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &chapters,
        chapter: Some(1),
        speed: 1.0,
    };
    let cx = Context {
        queue,
        book: Some(0),
        ..Context::new(&lib)
    };
    let mut b = Browser::default();
    b.press(&cx, Key::Select);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(b.press(&cx, Key::Left), Outcome::Seek(-30));
    let fifteen = Context {
        settings: Settings {
            jump: 15,
            ..Settings::default()
        },
        ..cx
    };
    assert_eq!(
        b.press(&fifteen, Key::Right),
        Outcome::Seek(15),
        "Jump Length"
    );
    assert_eq!(b.press(&cx, Key::R1), Outcome::Skip(1), "the next chapter");
    assert_eq!(b.press(&cx, Key::X), Outcome::Speed(0), "round the speeds");
    assert_eq!(b.press(&cx, Key::Y), Outcome::Bookmark);
    // Y held lists the marks, and B comes back.
    assert_eq!(b.mode(&cx, Key::Y), Mode::Hold);
    b.held(&cx, Key::Y);
    assert_eq!(b.view(), View::Bookmarks(0));
    b.press(&cx, Key::B);
    assert_eq!(b.view(), View::NowPlaying);
    b.press(&cx, Key::Select);
    assert_eq!(b.view(), View::Chapters);
    let page = b.page(&cx);
    assert_eq!(page.selected, 1, "on the one playing");
    assert!(page.rows[0].faint && page.rows[1].icon.is_some());
    assert_eq!(page.values[0].as_deref(), Some("0:30"), "how long each is");
    assert_eq!(
        page.values[1].as_deref(),
        Some("0:30"),
        "the last, to its part's end"
    );
    b.press(&cx, Key::Up);
    assert_eq!(b.press(&cx, Key::A), Outcome::Chapter(0));
    assert_eq!(b.view(), View::NowPlaying, "back to it");
    // A slip can be undone from the menu, which offers it first.
    let cx = Context {
        back: Some(754.0),
        ..cx
    };
    b.press(&cx, Key::Start);
    let page = b.page(&cx);
    assert_eq!(page.rows[0].text, "Back to 12:34");
    assert_eq!(b.press(&cx, Key::A), Outcome::Back);
}

#[test]
fn a_held_on_the_music_menu_shuffles_a_list_or_plays_on_the_latest_book() {
    use slot_music::books::{Place, Shelf};
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
    shelf.set_place(&lib, 0, begun);
    let cx = Context {
        shelf: &shelf,
        ..Context::new(&lib)
    };
    // Shuffle Library, Playlists, Audiobooks, Artists, Albums, Songs.
    let mut b = Browser::default();
    for _ in 0..5 {
        b.press(&cx, Key::Down);
    }
    assert_eq!(b.mode(&cx, Key::A), Mode::Hold);
    let Outcome::Play {
        tracks, shuffle, ..
    } = b.held(&cx, Key::A)
    else {
        panic!("Songs held plays");
    };
    assert_eq!((tracks.len(), shuffle), (2, Shuffle::Tracks));
    assert_eq!(b.view(), View::NowPlaying);
    let mut b = Browser::default();
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    assert_eq!(
        b.held(&cx, Key::A),
        Outcome::PlayBook {
            book: 0,
            over: false
        }
    );
    // In Audiobooks, Y lists a book's bookmarks.
    let mut b = Browser::default();
    b.press(&cx, Key::Down);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    b.press(&cx, Key::Y);
    assert_eq!(b.view(), View::Bookmarks(0));
}

#[test]
fn x_held_on_now_playing_in_the_music_menu_stops_it_all() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let tracks = [0];
    let queue = QueueView {
        tracks: &tracks,
        at: 0,
        playing: false,
        waiting: false,
        chapters: &[],
        chapter: None,
        speed: 1.0,
    };
    let cx = Context {
        queue,
        ..Context::new(&lib)
    };
    let mut b = Browser::default();
    assert_eq!(b.mode(&cx, Key::X), Mode::Hold);
    assert_eq!(b.press(&cx, Key::X), Outcome::Tell(Notice::Hold("X")));
    assert_eq!(b.held(&cx, Key::X), Outcome::Clear);
    // Nothing queued, nothing to stop.
    let idle = Context::new(&lib);
    assert_eq!(b.mode(&idle, Key::X), Mode::Once);
    // Stopped, the queue is gone, as before anything played.
    let mut p = Player::default();
    p.play(&lib, Queue::new(vec![0], 0, None));
    p.stop();
    assert!(p.queue().tracks.is_empty() && !p.resumable());
}
