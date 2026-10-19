//! The queue: opened at what plays, playing from it, taking out, carrying and its menu.

use super::common::*;

#[test]
fn the_queue_opens_on_what_plays_and_a_jumps_within_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2]), 1, None));
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
fn the_queue_opens_on_the_playing_track_and_x_held_goes_on_taking_tracks_out() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = ids(&[0, 1]);
    let queue = QueueView {
        tracks: &tracks,
        at: 1,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, Settings::default()), Key::A);
    // The queue, from Now Playing with SELECT.
    b.press(&cx(&lib, queue, Settings::default()), Key::Select);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&cx(&lib, queue, Settings::default()));
    assert_eq!(page.rows[0], "one");
    assert_eq!(page.selected, 1, "the playing track");
    let s = Settings::default();
    assert_eq!(b.mode(&cx(&lib, queue, s), Key::X), Mode::Repeat);
    assert_eq!(
        b.press(&cx(&lib, queue, s), Key::X),
        Outcome::Remove { at: 1, n: 1 }
    );
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::X),
        Outcome::None,
        "it stops at the last"
    );
    b.press(&cx(&lib, queue, s), Key::Up);
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::X),
        Outcome::Remove { at: 0, n: 1 }
    );
    // Held on, it takes out more at once.
    let n = (0..60).fold(0, |_, _| match b.held(&cx(&lib, queue, s), Key::X) {
        Outcome::Remove { n, .. } => n,
        _ => 0,
    });
    assert_eq!(n, 4, "one more each 20 repeats");
    b.press(&cx(&lib, queue, s), Key::X);
    assert_eq!(
        b.held(&cx(&lib, queue, s), Key::X),
        Outcome::Remove { at: 0, n: 1 },
        "a fresh press starts slow again"
    );
    b.press(&cx(&lib, queue, s), Key::Start);
    assert_eq!(b.view(), View::QueueMenu, "what no key does");
}

#[test]
fn the_queue_opens_below_now_playing_and_a_pick_goes_back_to_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = ids(&[0, 1]);
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
    b.press(&cx(&lib, queue, s), Key::Select);
    assert_eq!(b.view(), View::Queue);
    b.press(&cx(&lib, queue, s), Key::Down);
    assert_eq!(b.press(&cx(&lib, queue, s), Key::A), Outcome::Jump(1));
    assert_eq!(b.view(), View::NowPlaying);
    b.press(&cx(&lib, queue, s), Key::B);
    assert_eq!(b.view(), View::Main, "no pile of screens behind it");
}

#[test]
fn in_the_queue_y_favorites_a_track_and_held_lifts_it_to_carry() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let tracks = ids(&[0, 1, 2]);
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
    press(&mut b, Key::Select);
    assert_eq!(
        press(&mut b, Key::R1),
        Outcome::None,
        "the shoulders turn pages"
    );
    assert_eq!(press(&mut b, Key::L1), Outcome::None);
    press(&mut b, Key::Down);
    assert_eq!(
        press(&mut b, Key::Y),
        Outcome::Favorite(TrackId(1)),
        "as in every list"
    );
    assert_eq!(b.held(&cx(&lib, queue, s), Key::Y), Outcome::None, "lifted");
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
    let tracks = ids(&[0, 1, 2]);
    let queue = QueueView {
        tracks: &tracks,
        at: 1,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, s), Key::Select);
    b.press(&cx(&lib, queue, s), Key::Select);
    let page = b.page(&cx(&lib, queue, s));
    // The whole queue, at what plays, the played one faint above it.
    assert_eq!(page.rows.len(), 3);
    assert!(page.rows[0].faint && page.rows[1].playing && !page.rows[2].faint);
    // The last track, lifted and put next: just after the one playing.
    b.press(&cx(&lib, queue, s), Key::Down);
    b.held(&cx(&lib, queue, s), Key::Y);
    assert_eq!(b.press(&cx(&lib, queue, s), Key::R1), Outcome::Move(2, 2));
    assert_eq!(
        b.press(&cx(&lib, queue, s), Key::X),
        Outcome::Remove { at: 2, n: 1 }
    );
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
fn up_next_s_menu_saves_the_queue_as_a_playlist_or_clears_all_but_what_plays() {
    let (d, mut lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut q = Queue::new(ids(&[0, 1, 2]), 1, Some(7));
    fn view(q: &Queue) -> QueueView<'_> {
        QueueView {
            tracks: q.tracks(),
            at: q.at(),
            playing: true,
            waiting: false,
            speed: 1.0,
        }
    }
    let mut b = Browser::default();
    b.press(&cx(&lib, view(&q), Settings::default()), Key::A);
    b.press(&cx(&lib, view(&q), Settings::default()), Key::Select);
    assert_eq!(b.view(), View::Queue);
    b.press(&cx(&lib, view(&q), Settings::default()), Key::Start);
    assert_eq!(b.view(), View::QueueMenu);
    let saved = b.press(&cx(&lib, view(&q), Settings::default()), Key::A);
    assert_eq!((saved, b.view()), (Outcome::SaveQueue, View::Queue));
    let playing = q.current();
    let store = Store::open(d.path());
    for tracks in [q.tracks().to_vec(), ids(&[0])] {
        let p = lib.add_playlist("Saved Queue", tracks).unwrap();
        store.write_playlist(&lib.playlists[p], &lib);
    }
    store.flush();
    let names: Vec<&str> = lib.playlists.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Favorites", "Saved Queue", "Saved Queue 2"]);
    let again = Library::scan(d.path());
    assert_eq!(
        again.playlists[1].tracks,
        q.tracks(),
        "written, in the queue's order"
    );
    for key in [Key::Start, Key::Down] {
        b.press(&cx(&lib, view(&q), Settings::default()), key);
    }
    let cleared = b.press(&cx(&lib, view(&q), Settings::default()), Key::A);
    assert_eq!(cleared, Outcome::Clear);
    q.clear();
    assert_eq!(
        (q.tracks(), q.current()),
        (&playing.into_iter().collect::<Vec<_>>()[..], playing)
    );
    q.toggle_shuffle(3);
    assert_eq!(
        q.tracks().len(),
        1,
        "the order to go back to is cleared too"
    );
}

#[test]
fn select_opens_the_whole_queue_at_what_plays_played_songs_faint() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let tracks = ids(&[0, 1]);
    let queue = QueueView {
        tracks: &tracks,
        at: 1,
        playing: true,
        waiting: false,
        speed: 1.0,
    };
    let s = Settings::default();
    let mut b = Browser::default();
    b.press(&cx(&lib, queue, s), Key::Select);
    b.press(&cx(&lib, queue, s), Key::Select);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&cx(&lib, queue, s));
    assert_eq!(page.rows.len(), 2);
    assert!(page.rows[0].faint, "played");
    assert_eq!(page.selected, 1, "on what plays");
}

#[test]
fn a_song_queued_twice_while_shuffled_is_taken_out_and_put_back_as_the_copy_it_is() {
    let mut q = Queue::new(ids(&[0, 1, 2, 3]), 0, Some(7));
    // Added just after the one playing.
    q.enqueue(&ids(&[2]));
    assert_eq!(q.tracks()[1], TrackId(2));
    q.remove(1);
    q.toggle_shuffle(1);
    assert_eq!(
        q.tracks(),
        ids(&[0, 1, 2, 3]),
        "the order before the shuffle"
    );
    // Playing the added copy, unshuffling keeps playing it, not the first.
    let mut q = Queue::new(ids(&[0, 1, 2]), 0, None);
    q.toggle_shuffle(5);
    q.enqueue(&ids(&[1]));
    q.go(1);
    q.toggle_shuffle(5);
    assert_eq!(q.tracks(), ids(&[0, 1, 2, 1]));
    assert_eq!(q.at(), 3, "the copy that plays");
}
