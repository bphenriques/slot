use std::path::{Path, PathBuf};

use slot_music::{
    parse, track_title, Browser, Command, Event, Key, Library, MuseState, Outcome, Player, Queue,
    Settings, View,
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

fn names(lib: &Library) -> Vec<&str> {
    lib.artists.iter().map(|a| a.name.as_str()).collect()
}

#[test]
fn the_library_is_artist_album_track_folders_in_shelf_order() {
    let (_d, lib) = library(&[
        "Aerosmith/Toys in the Attic/02 Uncle Salty.mp3",
        "Aerosmith/Toys in the Attic/01 Toys in the Attic.mp3",
        "Aerosmith/Get a Grip/01 Intro.flac",
        "3 Doors Down/Away from the Sun/01 When I'm Gone.mp3",
        "Air Traffic/cover.jpg",
        "Loose.mp3",
        ".hidden/x/1.mp3",
    ]);
    assert_eq!(names(&lib), ["3 Doors Down", "Aerosmith"]);
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
        .map(|&k| b.press(lib, Queue::default(), &Settings::default(), k))
        .last()
        .unwrap_or(Outcome::None)
}

// The main screen: Now Playing, Artists, Albums, Tracks, Settings. Artists and Albums open on
// Play All and Shuffle, so these go on to the first of them.
const TO_ARTISTS: [Key; 4] = [Key::Down, Key::A, Key::Down, Key::Down];
const TO_ALBUMS: [Key; 5] = [Key::Down, Key::Down, Key::A, Key::Down, Key::Down];
const TO_SETTINGS: [Key; 2] = [Key::Up, Key::A];
const TO_TRACKS: [Key; 4] = [Key::Down, Key::Down, Key::Down, Key::A];

#[test]
fn the_main_screen_keeps_its_rows_whether_or_not_anything_plays() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let b = Browser::default();
    let page = b.page(&lib, Queue::default(), &Settings::default());
    assert_eq!(
        page.rows,
        ["Now Playing", "Artists", "Albums", "Tracks", "Settings"]
    );
    assert_eq!(page.pages, 1, "everything fits one page");
}

#[test]
fn rescan_is_the_last_music_setting() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    assert_eq!(
        press(&mut b, &lib, &[Key::Up, Key::A, Key::Up, Key::A]),
        Outcome::Rescan
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
        let p = b.page(&lib, Queue::default(), &Settings::default());
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
    assert_eq!(b.page(&lib, Queue::default(), &Settings::default()).page, 1);
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
        played(p.play(&lib, vec![0, 1], 0, None)),
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
    p.play(&lib, (0..5).collect(), 0, None);
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
    p.play(&lib, vec![0], 0, None);
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
    p.play(&lib, vec![0, 1, 2, 3], 0, Some(42));
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
            path: path.clone(),
            at: 0.0
        }
        .to_string(),
        "PLAY\tpath=/m/A/x.mp3"
    );
    assert_eq!(Command::Seek(12.0).to_string(), "SEEK\tat=12.0");
    assert_eq!(
        Command::Cover {
            path,
            base: "/tmp/c/3".into()
        }
        .to_string(),
        "COVER\tpath=/m/A/x.mp3\tbase=/tmp/c/3"
    );
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
    assert_eq!(
        parse("COVER\tbase=/tmp/c/3\tfile=/tmp/c/3.jpg"),
        Some(Event::Cover {
            base: "/tmp/c/3".into(),
            file: Some("/tmp/c/3.jpg".into())
        })
    );
    assert_eq!(
        parse("COVER\tbase=/tmp/c/3\tfile="),
        Some(Event::Cover {
            base: "/tmp/c/3".into(),
            file: None
        })
    );
    assert_eq!(parse("META\ttitle=x"), None);
}

#[test]
fn the_music_stays_audible_until_muse_confirms_it_stopped() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.play(&lib, vec![0], 0, None);
    p.on(&lib, Event::State(MuseState::Playing));
    p.stop();
    assert!(!p.owns_audio(), "a game may start opening the codec");
    assert!(p.audible(), "but Muse has not let go yet");
    p.on(&lib, Event::State(MuseState::Stopped));
    assert!(!p.audible());
}

#[test]
fn x_adds_what_a_row_stands_for_to_the_queue() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(
        press(&mut b, &lib, &[Key::X]),
        Outcome::Enqueue(vec![0, 1, 2])
    );
    // X -> (Play All, Shuffle, A, B): album B
    let out = press(
        &mut b,
        &lib,
        &[Key::A, Key::Down, Key::Down, Key::Down, Key::X],
    );
    assert_eq!(out, Outcome::Enqueue(lib.albums[1].tracks.clone()));
    assert_eq!(b.view(), View::Artist(0), "adding is not opening");
}

#[test]
fn the_queue_opens_on_what_plays_and_a_jumps_within_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut p = Player::default();
    p.play(&lib, vec![0, 1, 2], 1, None);
    let mut b = Browser::default();
    b.press(&lib, p.queue(), &Settings::default(), Key::A);
    b.press(&lib, p.queue(), &Settings::default(), Key::Down);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&lib, p.queue(), &Settings::default());
    let row = &page.rows[page.selected];
    assert_eq!(
        (row.text.as_str(), row.after.as_str(), row.playing),
        ("two", "X", true)
    );
    b.press(&lib, p.queue(), &Settings::default(), Key::Down);
    assert_eq!(
        b.press(&lib, p.queue(), &Settings::default(), Key::A),
        Outcome::Jump(2)
    );
    assert_eq!(played(p.jump(&lib, 2)), lib.tracks[2].path);
    assert_eq!(p.place(), (3, 3), "the queue itself is unchanged");
}

#[test]
fn pausing_for_a_game_keeps_the_queue_for_later() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, vec![0, 1], 1, None);
    p.enqueue(&[0]);
    assert_eq!(p.pause(), Some(Command::Pause));
    assert_eq!((p.current(), p.place()), (Some(1), (2, 3)));
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
}

#[test]
fn select_goes_to_now_playing_and_back_to_where_it_was() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_ARTISTS);
    assert_eq!(press(&mut b, &lib, &[Key::Select]), Outcome::None);
    assert_eq!(b.view(), View::NowPlaying);
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::Artists);
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
    p.play(&lib, vec![0, 1], 1, None);
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
    let page = b.page(&lib, Queue::default(), &Settings::default());
    assert_eq!(
        page.rows,
        ["Screen Off", "Skip Tracks", "List Size", "Rescan Library"]
    );
    assert_eq!(
        page.values,
        [
            Some("Pause".to_string()),
            Some("D-pad".to_string()),
            Some("Normal".to_string()),
            None
        ]
    );
    assert_eq!(
        press(&mut b, &lib, &[Key::Right]),
        Outcome::Change(slot_music::Row::ScreenOff, 1)
    );
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Main);
}

#[test]
fn the_repeat_setting_is_kept_on_the_card() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(Settings::read(d.path()), Settings::default());
    let s = Settings {
        repeat: slot_music::Repeat::One,
        ..Settings::default()
    };
    s.write(d.path());
    assert_eq!(Settings::read(d.path()), s);
    assert_eq!(slot_music::Repeat::Off.step(-1), slot_music::Repeat::One);
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
            at: 42.5
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
fn the_queue_screen_starts_with_clear_and_opens_on_the_playing_track() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = [0, 1];
    let queue = Queue {
        tracks: &tracks,
        at: 1,
        playing: true,
    };
    let mut b = Browser::default();
    b.press(&lib, queue, &Settings::default(), Key::A);
    b.press(&lib, queue, &Settings::default(), Key::Down);
    let page = b.page(&lib, queue, &Settings::default());
    assert_eq!(page.rows[0], "Clear Up Next");
    assert_eq!(page.selected, 2, "the playing track, under the clear row");
    b.press(&lib, queue, &Settings::default(), Key::Up);
    b.press(&lib, queue, &Settings::default(), Key::Up);
    assert_eq!(
        b.press(&lib, queue, &Settings::default(), Key::A),
        Outcome::ClearUpNext
    );
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
fn repeat_all_deals_a_new_shuffle_each_time_round() {
    let files: Vec<String> = (1..=8).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.set_repeat(slot_music::Repeat::All);
    p.play(&lib, (0..8).collect(), 0, Some(7));
    let first: Vec<usize> = p.queue().tracks.to_vec();
    p.skip(&lib, 7);
    p.skip(&lib, 1);
    let mut second: Vec<usize> = p.queue().tracks.to_vec();
    assert_ne!(first, second, "a new order for the second time round");
    second.sort();
    assert_eq!(second, (0..8).collect::<Vec<_>>());
}

#[test]
fn the_player_says_when_the_queue_is_worth_keeping() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    assert!(!p.take_changed());
    p.play(&lib, vec![0], 0, None);
    assert!(p.take_changed() && !p.take_changed(), "once per change");
    p.on(&lib, Event::Pos { at: 3.0, len: 9.0 });
    assert!(!p.take_changed(), "the clock ticking is not one");
    p.pause();
    assert!(p.take_changed());
}

#[test]
fn now_playing_takes_y_for_repeat_x_for_shuffle_and_skips_as_set() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(press(&mut b, &lib, &[Key::Y]), Outcome::Repeat(1));
    assert_eq!(press(&mut b, &lib, &[Key::X]), Outcome::ShuffleQueue);
    assert_eq!(press(&mut b, &lib, &[Key::Right]), Outcome::Skip(1));
    assert_eq!(press(&mut b, &lib, &[Key::R1]), Outcome::Seek(10));
    let shoulders = Settings {
        skip_with_dpad: false,
        ..Settings::default()
    };
    let q = Queue::default();
    assert_eq!(b.press(&lib, q, &shoulders, Key::Right), Outcome::Seek(10));
    assert_eq!(b.press(&lib, q, &shoulders, Key::L1), Outcome::Skip(-1));
}

#[test]
fn shuffling_the_queue_keeps_the_track_playing_and_undoes_to_the_old_order() {
    let files: Vec<String> = (1..=6).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, (0..6).collect(), 3, None);
    p.toggle_shuffle(9);
    assert!(p.shuffled());
    assert_eq!(p.current(), Some(3));
    assert_eq!(p.queue().tracks[0], 3, "what plays comes first");
    p.toggle_shuffle(9);
    assert!(!p.shuffled());
    assert_eq!(p.queue().tracks, &[0, 1, 2, 3, 4, 5]);
    assert_eq!(p.current(), Some(3));
}

#[test]
fn lists_count_their_items_and_not_the_rows_above_them() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_TRACKS);
    let page = |b: &Browser| b.page(&lib, Queue::default(), &Settings::default());
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
    let queue = Queue {
        tracks: &current,
        at: 0,
        playing: true,
    };
    let mut b = Browser::default();
    // The albums of everyone name whose they are.
    press(&mut b, &lib, &TO_ALBUMS);
    let page = b.page(&lib, queue, &settings);
    assert_eq!(
        (page.rows[2].text.as_str(), page.rows[2].after.as_str()),
        ("Get a Grip", "Aerosmith")
    );
    // An album's own tracks go by their numbers, and what plays is marked.
    b.press(&lib, queue, &settings, Key::A);
    let page = b.page(&lib, queue, &settings);
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
    let page = Browser::default().page(&lib, Queue::default(), &Settings::default());
    let values: Vec<Option<&str>> = page.values.iter().map(Option::as_deref).collect();
    assert_eq!(
        values,
        [None, Some("1"), Some("2"), Some("2"), None],
        "Now Playing, Artists, Albums, Tracks, Settings"
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
    let queue = Queue {
        tracks: &tracks,
        at: 0,
        playing: true,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&lib, queue, &s, Key::Select);
    b.press(&lib, queue, &s, Key::Down);
    assert_eq!(b.view(), View::Queue);
    b.press(&lib, queue, &s, Key::Down);
    assert_eq!(b.press(&lib, queue, &s, Key::A), Outcome::Jump(1));
    assert_eq!(b.view(), View::NowPlaying);
    b.press(&lib, queue, &s, Key::B);
    assert_eq!(b.view(), View::Main, "no pile of screens behind it");
}

#[test]
fn up_on_now_playing_opens_its_album_on_the_track_and_y_in_a_list_plays_next() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/B/01 three.mp3"]);
    let tracks = [1];
    let queue = Queue {
        tracks: &tracks,
        at: 0,
        playing: true,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&lib, queue, &s, Key::Select);
    b.press(&lib, queue, &s, Key::Up);
    assert_eq!(b.view(), View::Album(0));
    let page = b.page(&lib, queue, &s);
    assert_eq!(page.rows[page.selected], "two");
    assert_eq!(b.press(&lib, queue, &s, Key::Y), Outcome::PlayNext(vec![1]));
    b.press(&lib, queue, &s, Key::B);
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn play_next_goes_right_after_what_plays_and_survives_unshuffling() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, vec![0, 1, 2], 0, None);
    p.play_next(&[4]);
    assert_eq!(p.queue().tracks, &[0, 4, 1, 2]);
    p.toggle_shuffle(3);
    p.play_next(&[3]);
    p.toggle_shuffle(3);
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
        let rows = slot_music::rows(&lib, Queue::default(), view);
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
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A, Key::Down]);
    assert_eq!(b.page(&lib, Queue::default(), &s).rows[1], "Shuffle Albums");
    let out = press(&mut b, &lib, &[Key::A]);
    let Outcome::Play {
        tracks, shuffle, ..
    } = out
    else {
        panic!("{out:?}");
    };
    assert_eq!(shuffle, slot_music::Shuffle::Groups(vec![2, 1]));
    let mut p = Player::default();
    p.play_groups(&lib, tracks, &[2, 1], 5);
    let q = p.queue().tracks.to_vec();
    let a_together = q.windows(2).any(|w| w == [0, 1]);
    assert!(a_together, "album A plays whole and in order: {q:?}");
}

#[test]
fn the_queue_can_lose_move_and_clear_tracks_around_what_plays() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, vec![0, 1, 2, 3, 4], 2, None);
    p.remove(2);
    assert_eq!(p.queue().tracks, &[0, 1, 2, 3, 4], "not the one playing");
    p.remove(0);
    assert_eq!(
        (p.queue().tracks, p.current()),
        (&[1, 2, 3, 4][..], Some(2))
    );
    assert_eq!(p.move_track(3, -1), 2);
    assert_eq!(p.queue().tracks, &[1, 2, 4, 3]);
    assert_eq!(p.current(), Some(2));
    p.clear_up_next();
    assert_eq!(p.queue().tracks, &[1, 2]);
    assert_eq!(p.current(), Some(2));
}

#[test]
fn in_the_queue_x_removes_the_shoulders_move_and_what_played_is_dim() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let tracks = [0, 1, 2];
    let queue = Queue {
        tracks: &tracks,
        at: 1,
        playing: true,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&lib, queue, &s, Key::Select);
    b.press(&lib, queue, &s, Key::Down);
    let page = b.page(&lib, queue, &s);
    assert!(page.rows[1].dim && !page.rows[2].dim);
    assert_eq!(b.press(&lib, queue, &s, Key::R1), Outcome::Move(1, 1));
    assert_eq!(
        b.page(&lib, queue, &s).selected,
        3,
        "the cursor goes with it"
    );
    assert_eq!(b.press(&lib, queue, &s, Key::X), Outcome::Remove(2));
}

#[test]
fn the_list_size_sets_how_many_rows_a_page_holds() {
    use slot_music::ListSize;
    assert_eq!(
        [ListSize::Small, ListSize::Normal, ListSize::Large].map(ListSize::page),
        [10, 8, 7]
    );
    assert_eq!(ListSize::Normal.step(1), ListSize::Large);
    assert_eq!(
        ListSize::Large.step(1),
        ListSize::Large,
        "it stops at the ends"
    );
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
    p.play(&lib, vec![0, 1], 1, None);
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
fn a_on_list_size_goes_round_while_left_and_right_stop_at_the_ends() {
    use slot_music::ListSize;
    assert_eq!(ListSize::Large.step(0), ListSize::Small);
    assert_eq!(ListSize::Small.step(-1), ListSize::Small);
}

#[test]
fn the_now_playing_row_says_resume_when_paused_and_dims_with_nothing_queued() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let s = Settings::default();
    let b = Browser::default();
    let row = |queue: Queue| b.page(&lib, queue, &s).rows[0].clone();
    assert!(row(Queue::default()).dim, "nothing queued");
    let tracks = [0];
    let playing = Queue {
        tracks: &tracks,
        at: 0,
        playing: true,
    };
    assert_eq!(
        (row(playing).text.as_str(), row(playing).dim),
        ("Now Playing", false)
    );
    let paused = Queue {
        playing: false,
        ..playing
    };
    assert_eq!(row(paused).text, "Resume");
}
