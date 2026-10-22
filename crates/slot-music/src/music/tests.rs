//! The music player as slot drives it, as niri tests its layout: a card made for the test, keys
//! and the player's answers in, the `Screen` and what the player was told out, on the test's own
//! clock, the state checked whole after every step. Nothing is shared between tests: each has
//! its card, its clock and its player.
//!
//! The player is a fake that does as it is told, as slot-player would, and answers as the test
//! says; slot-player's own tests hold the real one to a device. A few tests below look at what
//! the state asks the world for (`Effect`s), which is its output as much as the screen is.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use super::controller::{Button, Input};
use super::*;
use crate::browse::keys::Press;
use crate::browse::rows::rows;
use crate::browse::{Key, View};
use crate::library::{AlbumId, BookId, TrackId};
use crate::playback::player::Repeat;
use crate::playback::queue::Queue;
use crate::screens::{Body, NowPlayingBody, Screen};
use crate::text;
use slot_player::Status;

/// How long a file is, as the fake player has it.
const LEN: f64 = 100.0;
/// A frame, as the test steps its time.
const FRAME: Duration = Duration::from_millis(20);

/// What the fake player was told, and what it says.
#[derive(Default)]
struct Told {
    commands: Vec<Command>,
    status: Status,
    path: Option<PathBuf>,
    events: Vec<Event>,
}

/// A player that does as it is told, at once, and says what the test has it say.
#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Told>>);

impl engine::Engine for Fake {
    fn send(&self, command: Command) {
        let mut t = self.0.borrow_mut();
        let s = &mut t.status;
        match &command {
            Command::Play { at, .. } | Command::Cue { at, .. } => {
                let playing = matches!(command, Command::Play { .. });
                s.state = if playing {
                    PlayState::Playing
                } else {
                    PlayState::Paused
                };
                (s.at, s.len, s.sounding) = (*at, LEN, playing);
            }
            Command::Pause if s.state == PlayState::Playing => {
                (s.state, s.sounding) = (PlayState::Paused, false)
            }
            Command::Resume if s.state == PlayState::Paused => {
                (s.state, s.sounding) = (PlayState::Playing, true)
            }
            Command::Stop => *s = Status::default(),
            Command::Seek(at) => s.at = *at,
            _ => {}
        }
        if let Command::Play { path, .. } | Command::Cue { path, .. } = &command {
            t.path = Some(path.clone());
        }
        t.commands.push(command);
    }

    fn status(&self) -> Status {
        self.0.borrow().status
    }

    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
}

/// The music player on a card of its own, its player the fake, its time the test's.
struct Rig {
    music: Music,
    fake: Fake,
    waited: Rc<Cell<Duration>>,
    starts: Rc<Cell<usize>>,
    _card: tempfile::TempDir,
}

impl Rig {
    /// A card with `files` on it, empty, read into the library.
    fn new(files: &[&str]) -> Rig {
        let card = tempfile::tempdir().unwrap();
        for f in files {
            let path = card.path().join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        // Help was seen on a first start before: the tests begin on the Music menu.
        std::fs::create_dir_all(card.path().join("Config")).unwrap();
        std::fs::write(card.path().join("Config/music.txt"), "help = seen\n").unwrap();
        let (fake, starts) = (Fake::default(), Rc::new(Cell::new(0)));
        let start: engine::Start = {
            let (fake, starts) = (fake.clone(), starts.clone());
            Box::new(move || {
                starts.set(starts.get() + 1);
                Box::new(fake.clone())
            })
        };
        let (first, waited) = (Instant::now(), Rc::new(Cell::new(Duration::ZERO)));
        let clock = {
            let waited = waited.clone();
            Box::new(move || {
                let w = waited.get();
                Now {
                    at: first + w,
                    wall: 1_700_000_000 + w.as_secs(),
                    seed: 1 + w.as_nanos() as u64,
                }
            })
        };
        let mut music = Music::with(card.path(), (64, 16), start, clock);
        // The library as the scan finds it, without waiting on its thread.
        let lib = Library::scan(card.path());
        music.update(Msg::Library(Found::Fresh(Box::new(lib), Vec::new())));
        let mut rig = Rig {
            music,
            fake,
            waited,
            starts,
            _card: card,
        };
        rig.told();
        rig
    }

    /// Taps each of `buttons`, a frame apart.
    fn keys(&mut self, buttons: &[Button]) {
        for &button in buttons {
            self.down(button);
            self.up(button);
            self.frames(FRAME);
        }
    }

    fn down(&mut self, button: Button) {
        self.music.input(Input::Down(button));
    }

    fn up(&mut self, button: Button) {
        self.music.input(Input::Up(button));
    }

    /// Holds `key` down for `ms`, a frame at a time, then lets it go.
    fn hold(&mut self, button: Button, ms: u64) {
        self.down(button);
        self.frames(Duration::from_millis(ms));
        self.up(button);
        self.frames(FRAME);
    }

    /// `ms` go by, a frame at a time.
    fn wait(&mut self, ms: u64) {
        self.frames(Duration::from_millis(ms));
    }

    fn frames(&mut self, by: Duration) {
        let end = self.waited.get() + by;
        while self.waited.get() < end {
            self.waited.set((self.waited.get() + FRAME).min(end));
            self.music.poll(false);
            self.check();
        }
    }

    /// What the player was told since last asked.
    fn told(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.fake.0.borrow_mut().commands)
    }

    /// The player says it is `at` seconds into a file `len` long, as it is.
    fn heard(&mut self, at: f64, len: f64) {
        {
            let status = &mut self.fake.0.borrow_mut().status;
            (status.at, status.len) = (at, len);
        }
        self.frames(FRAME);
    }

    /// The player tells `event`.
    fn says(&mut self, event: Event) {
        self.fake.0.borrow_mut().events.push(event);
        self.frames(FRAME);
    }

    /// The file playing ran out.
    fn ends(&mut self) {
        let path = self.fake.0.borrow().path.clone();
        if let Some(path) = path {
            self.fake.0.borrow_mut().status = Status::default();
            self.says(Event::End(path));
        }
    }

    fn screen(&self) -> Screen {
        self.music.screen()
    }

    fn now_playing(&self) -> NowPlayingBody {
        match self.screen().body {
            Body::NowPlaying(body) => body,
            body => panic!("not Now Playing: {body:?}"),
        }
    }

    /// The title of each row on the screen.
    fn rows(&self) -> Vec<String> {
        match self.screen().body {
            Body::List(list) => list.rows.into_iter().map(|r| r.text).collect(),
            _ => Vec::new(),
        }
    }

    /// The state whole, whatever came before: the queue and the cursor within what they hold,
    /// and a screen that says no more than there is.
    fn check(&self) {
        let m = &self.music;
        let queue = m.player.queue();
        assert!(
            queue.tracks.is_empty() || queue.at < queue.tracks.len(),
            "the queue at {} of {}",
            queue.at,
            queue.tracks.len()
        );
        let (view, cx) = (m.browser.view(), m.context());
        if !matches!(view, View::NowPlaying | View::Lyrics) {
            let none = rows(&cx, view).is_empty();
            assert!(
                none || m.browser.row(&cx).is_some(),
                "{view:?}: the cursor on no row"
            );
        }
        match m.screen().body {
            Body::List(list) => assert!(list.rows.is_empty() || list.selected < list.rows.len()),
            Body::NowPlaying(body) => {
                let p = body.progress.unwrap_or(0.0);
                assert!((0.0..=1.0).contains(&p), "progress {p}");
            }
            _ => {}
        }
    }
}

/// A book of two parts, and a song beside it.
const BOOK: [&str; 3] = [
    "Audiobooks/Kipling - Just So/01 whale.mp3",
    "Audiobooks/Kipling - Just So/02 camel.mp3",
    "Music/X/A/01 one.mp3",
];

impl Rig {
    /// Up or Down to the row named `row` on the page shown, as a listener would.
    fn to(&mut self, row: &str) {
        let Body::List(list) = self.screen().body else {
            panic!("no list for {row}");
        };
        let at = list.rows.iter().position(|r| r.text == row);
        let at = at.unwrap_or_else(|| panic!("no {row} in {:?}", self.rows()));
        let key = if at > list.selected {
            Button::Down
        } else {
            Button::Up
        };
        for _ in 0..at.abs_diff(list.selected) {
            self.keys(&[key]);
        }
    }

    /// From the Music menu, Library's row named `row`.
    fn library(&mut self, row: &str) {
        self.to(text::LIBRARY);
        self.keys(&[Button::A]);
        self.to(row);
    }

    /// From the Music menu, the book: Audiobooks, All Books, the first.
    fn play_book(&mut self) {
        self.to(text::BOOKS);
        self.keys(&[Button::A]);
        self.to("All Books");
        self.keys(&[Button::A, Button::A]);
        assert!(matches!(self.screen().body, Body::NowPlaying(_)));
    }

    /// Now Playing's START menu, on its row named `row`.
    fn menu(&mut self, row: &str) {
        self.keys(&[Button::Start]);
        self.to(row);
    }
}

#[test]
fn any_keys_in_any_order_keep_the_music_whole() {
    let mut rig = Rig::new(&[
        "Music/Abba/Gold/01 one.mp3",
        "Music/Abba/Gold/02 two.mp3",
        "Music/The Cure/Disintegration/01 plainsong.mp3",
        "Music/1999/Party/01 party.mp3",
        "Audiobooks/Kipling - Just So/01 whale.mp3",
        "Audiobooks/Kipling - Just So/02 camel.mp3",
    ]);
    let keys = [
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::Start,
        Button::Select,
    ];
    // The same every run: a sequence that breaks the state is found again.
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    for _ in 0..3000 {
        match next(13) {
            0..=6 => rig.keys(&[keys[next(keys.len() as u64) as usize]]),
            7 => rig.hold(keys[next(keys.len() as u64) as usize], next(900)),
            8 => rig.wait(next(3000)),
            9 => rig.ends(),
            10 => rig.heard(next(100) as f64, LEN),
            11 => rig.says(Event::Error("a file that will not play".into())),
            // SELECT with a shoulder: a chord, its two buttons as they come.
            _ => {
                rig.down(Button::Select);
                rig.keys(&[[Button::L1, Button::R1][next(2) as usize]]);
                rig.up(Button::Select);
            }
        }
    }
}

#[test]
fn a_book_the_sleep_timer_stops_goes_back_two_minutes() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(300.0, 600.0);
    rig.menu("Sleep Timer");
    rig.keys(&[Button::Right, Button::Right, Button::B]);
    rig.told();
    rig.wait(5 * 60_000 + 9_000);
    let told = rig.told();
    assert!(told.contains(&Command::Pause), "{told:?}");
    assert!(
        told.contains(&Command::Seek(180.0)),
        "two minutes back: {told:?}"
    );
}

#[test]
fn a_book_picks_up_a_little_back_the_longer_it_was_paused() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(100.0, 600.0);
    rig.keys(&[Button::A]);
    assert!(rig.told().contains(&Command::Pause));
    rig.keys(&[Button::A]);
    assert!(
        !rig.told().iter().any(|c| matches!(c, Command::Seek(_))),
        "a moment: none"
    );
    rig.keys(&[Button::A]);
    rig.wait(10 * 60_000);
    rig.keys(&[Button::A]);
    let told = rig.told();
    assert!(
        told.contains(&Command::Seek(85.0)),
        "ten minutes: 15 s back: {told:?}"
    );
}

#[test]
fn continue_plays_on_the_book_listened_to_last_from_where_it_was() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(42.0, 600.0);
    rig.keys(&[Button::A]);
    rig.hold(Button::B, 600);
    rig.to(text::BOOKS);
    rig.keys(&[Button::A]);
    assert_eq!(rig.rows()[0], text::CONTINUE);
    rig.told();
    rig.keys(&[Button::A]);
    let told = rig.told();
    let at = told.iter().find_map(|c| match c {
        Command::Play { at, .. } | Command::Cue { at, .. } => Some(*at),
        Command::Seek(at) => Some(*at),
        _ => None,
    });
    assert_eq!(at, Some(42.0), "where it was left: {told:?}");
    assert!(matches!(rig.screen().body, Body::NowPlaying(_)));
}

#[test]
fn under_the_lid_a_book_jumps_back_to_hear_again_and_a_trigger_held_marks_the_moment() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(40.0, 60.0);
    rig.music.set_dark(true);
    rig.told();
    rig.keys(&[Button::L1]);
    let jump = f64::from(rig.music.settings.jump);
    let told = rig.told();
    assert!(
        told.iter()
            .any(|c| matches!(c, Command::Seek(at) if (*at - (40.0 - jump)).abs() < 0.5)),
        "back as far as Jump Length: {told:?}"
    );
    rig.down(Button::R2);
    rig.wait(520);
    let buzz = rig.music.rumble();
    rig.up(Button::R2);
    assert_eq!(rig.told(), [], "held, not tapped: it plays on");
    rig.music.set_dark(false);
    assert_eq!(rig.now_playing().marks.len(), 1, "marked");
    assert_eq!(buzz, Some(u16::MAX), "a buzz says so, unseen");
}

#[test]
fn a_book_s_now_playing_shows_its_part_place_and_time_left() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(30.0, 60.0);
    let np = rig.now_playing();
    assert_eq!(np.title, "whale", "the part, a chapter of its own");
    assert_eq!((np.by.as_str(), np.from.as_str()), ("Kipling", "Just So"));
    assert_eq!(np.time, text::elapsed(30.0, 60.0));
    assert_eq!(np.progress, Some(0.5));
    // Where music says its place, the time left, here this part's, its parts' lengths not known.
    assert_eq!(np.place, text::time_left(30.0));
    // Up Next names the next chapter, as it names the next song; at 1×, no speed mark.
    assert_eq!(np.next, Some(text::up_next_chapter("camel")));
    assert_eq!(np.speed, None);
    rig.keys(&[Button::X]);
    assert_eq!(
        rig.now_playing().speed,
        Some(text::speed(1.25)),
        "faster: marked"
    );
    assert!(rig.screen().corner.is_empty(), "said once, by the mark");
    rig.keys(&[Button::X, Button::X, Button::X, Button::X, Button::X]);
    assert_eq!(rig.now_playing().speed, None, "back round to 1×");
    // A bookmark, ticked on the bar where it is.
    rig.keys(&[Button::Y]);
    assert_eq!(rig.screen().corner, text::notice(&Notice::Bookmarked));
    assert_eq!(rig.now_playing().marks, [0.5]);
    rig.hold(Button::Y, 600);
    assert_eq!(rig.screen().title, text::BOOKMARKS);
    assert_eq!(rig.rows().len(), 1);
}

#[test]
fn the_corner_says_a_notice_first_then_locked_then_the_sleep_timer() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    rig.menu("Sleep Timer");
    rig.keys(&[Button::Right, Button::Right, Button::B]);
    let sleeping = |rig: &Rig| rig.screen().corner.starts_with("Sleep");
    assert!(
        sleeping(&rig),
        "the timer, counting down: {}",
        rig.screen().corner
    );
    rig.music.tell(Notice::Saved);
    assert_eq!(rig.screen().corner, text::notice(&Notice::Saved));
    rig.wait(2000);
    assert!(sleeping(&rig), "the notice gone, the timer again");
    rig.down(Button::L1);
    rig.down(Button::R1);
    rig.wait(600);
    rig.up(Button::R1);
    rig.up(Button::L1);
    rig.wait(2000);
    assert_eq!(
        rig.screen().corner,
        text::notice(&Notice::Locked(true)),
        "over the timer"
    );
}

#[test]
fn a_list_of_books_says_so_for_its_empty_thumbnails_and_a_list_of_albums_does_not() {
    let mut rig = Rig::new(&BOOK);
    let books = |rig: &Rig| match rig.screen().body {
        Body::List(list) => list.thumbs && list.books,
        _ => false,
    };
    rig.to(text::BOOKS);
    rig.keys(&[Button::A]);
    rig.to("All Books");
    rig.keys(&[Button::A]);
    assert!(books(&rig), "a book, not a note");
    rig.hold(Button::B, 600);
    rig.library("Albums");
    rig.keys(&[Button::A]);
    assert!(!books(&rig), "a note");
}

#[test]
fn a_book_playing_through_update_library_plays_on_as_music_does() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(1.0, 60.0);
    rig.told();
    rig.music.rescan(false);
    let lib = Library::scan(&rig.music.root);
    rig.music
        .update(Msg::Library(Found::Fresh(Box::new(lib), Vec::new())));
    rig.wait(100);
    assert!(rig.music.book().is_some(), "the book is back");
    // Now Playing names nothing by number: it stays.
    assert_eq!(rig.now_playing().title, "whale", "and plays on");
    let told = rig.told();
    assert!(
        !told
            .iter()
            .any(|c| matches!(c, Command::Stop | Command::Play { .. })),
        "{told:?}"
    );
}

#[test]
fn a_player_gone_is_started_again_the_shell_s_way_to_play() {
    let mut rig = Rig::new(&BOOK[2..]);
    assert_eq!(rig.starts.get(), 1);
    rig.keys(&[Button::A]);
    rig.says(Event::Gone);
    assert_eq!(rig.starts.get(), 1, "none started until one is wanted");
    rig.keys(&[Button::A]);
    assert_eq!(rig.starts.get(), 2, "playing starts another");
    assert!(rig.told().iter().any(|c| matches!(c, Command::Play { .. })));
}

#[test]
fn the_pocket_combo_held_locks_the_keys_with_a_buzz_and_held_again_unlocks_them() {
    let mut rig = Rig::new(&BOOK[2..]);
    // On a list, the shoulders are its own: the combo is a pocket's, on Now Playing.
    rig.down(Button::L1);
    rig.down(Button::R1);
    rig.wait(600);
    rig.up(Button::R1);
    rig.up(Button::L1);
    assert!(!rig.music.buttons.pocket.locked, "not on a list");
    rig.keys(&[Button::A]);
    assert!(matches!(rig.screen().body, Body::NowPlaying(_)));
    let combo = |rig: &mut Rig, a, b| {
        rig.down(a);
        rig.down(b);
        rig.wait(600);
        rig.up(b);
        rig.up(a);
    };
    combo(&mut rig, Button::R1, Button::L1);
    assert!(rig.music.buttons.pocket.locked, "either order");
    assert_eq!(rig.music.rumble(), Some(u16::MAX), "a buzz says so");
    assert_eq!(rig.music.rumble(), None, "told once");
    assert_eq!(rig.screen().corner, text::notice(&Notice::Locked(true)));
    rig.told();
    rig.keys(&[Button::B, Button::Right, Button::A]);
    assert!(
        matches!(rig.screen().body, Body::NowPlaying(_)),
        "locked, still here"
    );
    assert!(rig.told().is_empty(), "and the player told nothing");
    // Once to lock, twice to unlock, each a long buzz as long as the gap after it.
    let pattern = |rig: &mut Rig| {
        let mut seen = vec![rig.music.rumble()];
        for _ in 0..10 {
            rig.waited.set(rig.waited.get() + pocket::LOCK_BUZZ / 2);
            seen.push(rig.music.rumble());
        }
        seen.into_iter().flatten().collect::<Vec<_>>()
    };
    rig.waited.set(rig.waited.get() + Duration::from_secs(5));
    assert_eq!(rig.music.rumble(), Some(0), "the motor stops");
    combo(&mut rig, Button::L2, Button::R2);
    assert!(!rig.music.buttons.pocket.locked, "either combo unlocks");
    assert_eq!(
        pattern(&mut rig),
        [u16::MAX, 0, u16::MAX, 0],
        "twice, to unlock"
    );
    // A lock lasts only as long as the screen is off.
    combo(&mut rig, Button::L1, Button::R1);
    rig.music.set_dark(true);
    rig.music.set_dark(false);
    assert!(!rig.music.buttons.pocket.locked, "the lid opened unlocks");
    // The screen going off forgets the keys held: one alone is no combo after.
    rig.down(Button::L1);
    rig.music.set_dark(true);
    rig.down(Button::R1);
    rig.wait(600);
    assert!(!rig.music.buttons.pocket.locked);
}

#[test]
fn under_the_lid_a_trigger_plays_or_pauses_a_shoulder_changes_song_and_locked_neither() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[
        Button::Down,
        Button::Down,
        Button::A,
        Button::A,
        Button::A,
        Button::A,
    ]);
    assert_eq!(rig.now_playing().place, text::of(1, 2));
    rig.music.set_dark(true);
    rig.told();
    rig.keys(&[Button::R2]);
    let first = rig.told();
    assert!(
        first == [Command::Pause] || first == [Command::Resume],
        "as A would: {first:?}"
    );
    rig.keys(&[Button::L2]);
    let second = rig.told();
    assert!(
        second.len() == 1 && second != first,
        "either trigger, back again: {second:?}"
    );
    rig.keys(&[Button::R1]);
    rig.music.set_dark(false);
    assert_eq!(rig.now_playing().place, text::of(2, 2), "a shoulder skips");
    rig.music.set_dark(true);
    // Locked, by the two together, a trigger does nothing.
    rig.down(Button::L2);
    rig.down(Button::R2);
    rig.wait(600);
    rig.up(Button::R2);
    rig.up(Button::L2);
    assert!(rig.music.buttons.pocket.locked);
    rig.told();
    rig.keys(&[Button::R2, Button::L1]);
    assert_eq!(rig.told(), [], "nothing changes in a pocket");
}

#[test]
fn it_is_due_to_change_only_while_it_moves_or_plays_where_it_is_seen() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.music.settings.screen_off = crate::settings::ScreenOff::Play;
    assert!(
        rig.music.due_in().is_none(),
        "nothing playing: only what comes in changes it"
    );
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
    assert!(rig.now_playing().status.playing);
    let due = rig
        .music
        .due_in()
        .expect("a song playing, its time and bar seen moving");
    assert!(!due.is_zero() && due <= Duration::from_secs(1), "{due:?}");
    rig.music.set_dark(true);
    assert!(rig.music.due_in().is_none(), "playing unseen");
    // Locked in the dark, the motor buzzes it, frame by frame, and then is still.
    rig.down(Button::L2);
    rig.down(Button::R2);
    rig.wait(600);
    rig.up(Button::R2);
    rig.up(Button::L2);
    assert!(
        rig.music.buttons.pocket.locked && rig.music.due_in() == Some(Duration::ZERO),
        "buzzing"
    );
    // The motor is read each frame, as slot does, for a second.
    let buzzed = |rig: &mut Rig| {
        for _ in 0..60 {
            rig.wait(16);
            rig.music.rumble();
        }
    };
    buzzed(&mut rig);
    assert!(rig.music.due_in().is_none(), "buzzed");
    rig.down(Button::L2);
    rig.down(Button::R2);
    rig.wait(600);
    rig.up(Button::R2);
    rig.up(Button::L2);
    buzzed(&mut rig);
    rig.music.set_dark(false);
    rig.keys(&[Button::A]);
    assert!(!rig.now_playing().status.playing);
    assert!(rig.music.due_in().is_none(), "paused, shown");
    // Played on, once its fade in is done.
    rig.keys(&[Button::A, Button::B]);
    rig.wait(1000);
    assert!(
        rig.music.player.state() == PlayState::Playing && rig.music.due_in().is_none(),
        "a list shows nothing of it moving"
    );
}

#[test]
fn the_pocket_combo_tapped_does_nothing_and_neither_key_acts_alone() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[
        Button::Down,
        Button::Down,
        Button::A,
        Button::A,
        Button::A,
        Button::A,
    ]);
    assert_eq!(rig.now_playing().place, text::of(1, 2));
    for (dark, first, second, up) in [
        (false, Button::L1, Button::R1, Button::L1),
        (false, Button::R2, Button::L2, Button::L2),
        (true, Button::L2, Button::R2, Button::L2),
        (true, Button::R1, Button::L1, Button::R1),
    ] {
        rig.music.set_dark(dark);
        rig.told();
        rig.down(first);
        rig.down(second);
        rig.up(up);
        rig.up(if up == first { second } else { first });
        let told = rig.told();
        assert_eq!(told, [], "{first:?} with {second:?}, dark {dark}");
        assert!(!rig.music.buttons.pocket.locked, "a tap is no lock");
    }
    rig.music.set_dark(false);
    assert_eq!(rig.now_playing().place, text::of(1, 2), "no song skipped");
}

#[test]
fn moving_fast_through_a_list_by_name_shows_its_letter_and_it_follows_each_move() {
    let mut rig = Rig::new(&[
        "Music/Abba/A/01 one.mp3",
        "Music/Beck/A/01 one.mp3",
        "Music/The Cure/A/01 one.mp3",
        "Music/Dido/A/01 one.mp3",
    ]);
    let artists = rig.music.browser.view() == View::Main;
    assert!(artists);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A]);
    assert_eq!(rig.screen().title, "Artists");
    rig.keys(&[Button::Down]);
    assert_eq!(rig.screen().letter, None, "a step at a time, none");
    rig.keys(&[Button::Right]);
    assert_eq!(rig.screen().letter, Some('D'), "a page on");
    rig.keys(&[Button::Up]);
    assert_eq!(
        rig.screen().letter,
        Some('C'),
        "then each step, The Cure under C"
    );
    rig.wait(1000);
    assert_eq!(rig.screen().letter, None, "gone after a moment");
    // Held: the press moves a step, the first repeat another, the letter shown.
    rig.hold(Button::Up, 450);
    assert_eq!(rig.screen().letter, Some('A'), "held");
}

#[test]
fn up_and_down_on_now_playing_do_nothing_and_start_s_menu_sets_the_repeat() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    let status = |rig: &Rig| rig.now_playing().status;
    let before = status(&rig);
    rig.keys(&[Button::Up, Button::Down]);
    assert_eq!(status(&rig), before, "as on other players");
    assert!(rig.screen().corner.is_empty(), "and nothing said");
    rig.menu("Repeat");
    rig.keys(&[Button::Right]);
    rig.keys(&[Button::B]);
    assert_eq!(status(&rig).repeat, Repeat::All, "the menu's row");
}

// What the state asks the world for: its effects, before the shell carries them out.

/// The jobs the state gave the worker, of `kind`, by the song they were for.
fn jobs(music: &Music, kind: fn(&Job) -> Option<TrackId>) -> Vec<TrackId> {
    let jobs = music.out.iter().filter_map(|e| match e {
        Effect::Work(_, job) => kind(job),
        _ => None,
    });
    jobs.collect()
}

fn chapters_of(job: &Job) -> Option<TrackId> {
    match job {
        Job::Chapters { track, .. } => Some(*track),
        _ => None,
    }
}

fn lyrics_of(job: &Job) -> Option<TrackId> {
    match job {
        Job::Lyrics { track, .. } => Some(*track),
        _ => None,
    }
}

#[test]
fn each_part_of_a_book_has_its_chapters_asked_for_once_however_it_is_reached() {
    let mut rig = Rig::new(&[
        "Audiobooks/Wilde - Tales/01 one.m4b",
        "Audiobooks/Wilde - Tales/02 two.m4b",
    ]);
    let music = &mut rig.music;
    let play = music.play_book(BookId(0), false);
    music.send(play);
    music.read_chapters();
    music.read_chapters();
    let parts = music.library.albums[music.library.books[0].album]
        .tracks
        .clone();
    assert_eq!(
        jobs(music, chapters_of),
        parts,
        "this part's and the next's, once"
    );
    // On into the next part as the first runs out, as a gapless book goes.
    let end = Event::End(music.library.tracks[parts[0]].path.clone());
    music.player.on_event(&music.library, end);
    music.read_chapters();
    assert_eq!(
        jobs(music, chapters_of),
        parts,
        "the next part's already asked"
    );
    let chapters = vec![crate::books::chapters::Chapter {
        start: 0.0,
        title: "One".into(),
    }];
    music.player.set_chapters(parts[1], chapters);
    assert_eq!(music.player.chapters(), 1, "there as the part plays");
}

#[test]
fn a_song_s_lyrics_and_the_next_s_are_asked_for_once_and_shown_as_they_come() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    std::fs::write(
        rig._card.path().join("Music/X/A/01 one.lrc"),
        "[00:01.00]Hello",
    )
    .unwrap();
    let music = &mut rig.music;
    let tracks = music.library.albums[AlbumId(0)].tracks.clone();
    let play = music
        .player
        .play(&music.library, Queue::new(tracks.clone(), 0, None));
    music.send(play);
    music.browser.push(View::NowPlaying, 0);
    music.out.clear();
    music.read_lyrics();
    assert_eq!(
        jobs(music, lyrics_of),
        tracks,
        "the song playing and the next"
    );
    assert!(music.lyrics_now().is_none(), "asked, not waited for");
    // The worker's answers, as it reads them.
    for &t in &tracks {
        let read = crate::lyrics::read(&music.library.tracks[t].path);
        music.update(Msg::Worker(music.generation, Done::Lyrics(t, read)));
    }
    assert!(music.lyrics_now().is_some(), "beside the song");
    music.out.clear();
    music.read_lyrics();
    assert!(
        jobs(music, lyrics_of).is_empty(),
        "once a song: not read again"
    );
    music.player.skip(&music.library, 1);
    music.read_lyrics();
    assert!(
        music.lyrics_now().is_none(),
        "the next has none, already read"
    );
}

#[test]
fn with_the_screen_off_no_lyrics_or_pictures_are_read() {
    let mut rig = Rig::new(&BOOK[2..]);
    // Played, the lid closed before a frame could read for it.
    rig.down(Button::A);
    rig.up(Button::A);
    rig.music.set_dark(true);
    let frame = |rig: &mut Rig| {
        rig.music.out.clear();
        let status = engine::Engine::status(&rig.fake);
        rig.music.frame(status, Vec::new(), false);
        rig.music.out.iter().any(|e| matches!(e, Effect::Work(..)))
    };
    assert!(!frame(&mut rig), "nothing read for a screen that is off");
    rig.music.set_dark(false);
    assert!(frame(&mut rig), "and read as soon as it is on");
}

#[cfg(feature = "perf")]
#[test]
fn the_performance_log_runs_only_while_it_is_on_and_counts_files_opened() {
    let mut rig = Rig::new(&[]);
    let music = &mut rig.music;
    assert!(music.shell.perf.is_none(), "off by default: nothing kept");
    music.out.push(Effect::PerfLog(true));
    music.run();
    music.poll(false);
    let stats = music.shell.perf.as_ref().map(|p| p.stats.clone()).unwrap();
    assert!(!stats.what.lock().unwrap().is_empty(), "the frame noted");
    let play = Command::Play {
        path: "a.mp3".into(),
        at: 0.0,
        speed: 1.0,
    };
    music
        .out
        .extend([Effect::Player(play), Effect::Player(Command::Pause)]);
    music.run();
    assert_eq!(stats.opened.load(std::sync::atomic::Ordering::Relaxed), 1);
    music.out.push(Effect::PerfLog(false));
    music.run();
    assert!(music.shell.perf.is_none());
}

#[test]
fn the_sound_settings_are_told_once_while_they_stand() {
    let mut rig = Rig::new(&[]);
    let music = &mut rig.music;
    music.tell_sound();
    let told = music.out.len();
    assert!(told > 0, "the sound, once");
    music.tell_sound();
    assert_eq!(music.out.len(), told, "not again while it stands");
    music.run();
    assert!(music.out.is_empty(), "carried out");
    // A player started afresh, its last gone, is told it all again, mono and equalizer too.
    music.on_player(Event::Gone);
    let play = music.player.play(&music.library, Queue::default());
    music.send(play.or(Some(Command::Resume)));
    music.tell_sound();
    let again = music
        .out
        .iter()
        .filter(|e| matches!(e, Effect::Player(Command::Mono(_) | Command::Equalizer(_))));
    assert_eq!(again.count(), 2);
}

#[test]
fn a_tap_and_a_hold_reach_the_screens_as_presses() {
    // The keys' timing is theirs; what a press does is the screens'.
    let mut rig = Rig::new(&BOOK[2..]);
    assert!(!rig.music.act(Press::Tap(crate::browse::Key::Down)));
    assert_eq!(rig.music.browser.at(), 1);
}

#[test]
fn select_tapped_goes_to_now_playing_and_with_a_shoulder_skips_instead() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::A]);
    rig.hold(Button::B, 600);
    assert_eq!(rig.screen().title, text::MUSIC);
    rig.told();
    rig.down(Button::Select);
    rig.keys(&[Button::R1]);
    rig.up(Button::Select);
    rig.wait(100);
    assert!(
        rig.told().iter().any(|c| matches!(c, Command::Play { .. })),
        "skipped"
    );
    let next = text::notice(&Notice::Skipped { on: true });
    assert_eq!(
        rig.screen().corner,
        next,
        "away from Now Playing, the corner says so"
    );
    assert_eq!(
        rig.screen().title,
        text::MUSIC,
        "SELECT's own tap taken by the chord"
    );
    rig.keys(&[Button::Select]);
    assert!(
        matches!(rig.screen().body, Body::NowPlaying(_)),
        "tapped alone, Now Playing"
    );
    rig.wait(2000);
    rig.down(Button::Select);
    rig.keys(&[Button::L1]);
    rig.up(Button::Select);
    assert_eq!(rig.screen().corner, "", "Now Playing shows the song itself");
}

#[test]
fn a_pause_keeps_the_volume_for_the_player_s_own_fade_out() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    rig.wait(1000);
    assert_eq!(rig.music.volume_scale(), 1.0, "faded in, full");
    rig.keys(&[Button::A]);
    assert!(rig.told().contains(&Command::Pause));
    assert_eq!(
        rig.music.volume_scale(),
        1.0,
        "not cut to nothing under the fade out"
    );
}

#[test]
fn shuffling_keeps_the_song_s_lyrics_and_its_hint_in_the_legend() {
    let files: Vec<String> = (1..=6)
        .map(|i| format!("Music/X/A/{i:02} s{i}.mp3"))
        .collect();
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut rig = Rig::new(&refs);
    for i in 1..=6 {
        let lrc = rig._card.path().join(format!("Music/X/A/{i:02} s{i}.lrc"));
        std::fs::write(lrc, "[00:01.00]Hello").unwrap();
    }
    rig.keys(&[Button::A]);
    let lyrics = |rig: &Rig| {
        matches!(
            rig.screen().legend,
            crate::screens::Legend::NowPlaying { lyrics: true, .. }
        )
    };
    // The worker reads them on its own thread: a moment of real time for it.
    for _ in 0..200 {
        if lyrics(&rig) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        rig.wait(20);
    }
    assert!(lyrics(&rig), "read, and hinted");
    for _ in 0..6 {
        rig.keys(&[Button::Down]);
        // The new next song's lyrics come in meanwhile: the hint stays through every frame.
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(2));
            rig.wait(20);
            assert!(lyrics(&rig), "the hint stays as the next song changes");
        }
    }
}

#[test]
fn the_pocket_combo_as_the_device_logged_it_neither_seeks_nor_skips() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::A]);
    assert!(matches!(rig.screen().body, Body::NowPlaying(_)));
    rig.told();
    // From slot.log, 2026-10-18: R1, R2 a frame later, R2 up after 333 ms, R1 66 ms after.
    rig.down(Button::R1);
    rig.wait(16);
    rig.down(Button::R2);
    rig.wait(333);
    rig.up(Button::R2);
    rig.wait(66);
    rig.up(Button::R1);
    assert_eq!(rig.told(), [], "nothing: a tap of the two is no key's");
}

#[test]
fn locked_the_combo_tapped_does_nothing_and_only_held_unlocks() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    let combo = |rig: &mut Rig, ms| {
        rig.down(Button::R1);
        rig.down(Button::R2);
        rig.wait(ms);
        rig.up(Button::R2);
        rig.up(Button::R1);
    };
    combo(&mut rig, 600);
    assert!(rig.music.buttons.pocket.locked);
    rig.told();
    combo(&mut rig, 100);
    assert_eq!(rig.told(), [], "a squeeze in a pocket changes nothing");
    assert!(rig.music.buttons.pocket.locked);
    combo(&mut rig, 600);
    assert!(!rig.music.buttons.pocket.locked, "held, it unlocks");
}

/// How many of the frames in `secs` showed a screen other than the one before: the frames worth
/// drawing. Playing, the player moves on with the clock, as a real one does.
fn seen_changes(rig: &mut Rig, secs: u64) -> usize {
    let mut last = rig.screen();
    let mut changed = 0;
    for _ in 0..secs * 1000 / FRAME.as_millis() as u64 {
        {
            let status = &mut rig.fake.0.borrow_mut().status;
            if status.state == PlayState::Playing {
                status.at += FRAME.as_secs_f64();
            }
        }
        rig.frames(FRAME);
        let screen = rig.screen();
        changed += usize::from(screen != last);
        last = screen;
    }
    changed
}

/// Plays on `ms` milliseconds, one at a time, the song heard at `speed`, as the frontend's loop
/// would: resting until the moment `due_in` names, then looking, and asking again. The screen
/// must never change while the loop rests but in the millisecond `due_in` aims past a step. Says
/// how many changes there were, and how many wakes found nothing new.
fn rests_never_late(rig: &mut Rig, ms: u64, speed: f64) -> (usize, usize) {
    let mut last = rig.screen();
    let mut wake = rig.music.due_in().map(|d| rig.waited.get() + d);
    let (mut changes, mut idle) = (0, 0);
    for _ in 0..ms {
        {
            let status = &mut rig.fake.0.borrow_mut().status;
            if status.state == PlayState::Playing {
                status.at += 0.001 * speed;
            }
        }
        rig.frames(Duration::from_millis(1));
        let (screen, now) = (rig.screen(), rig.waited.get());
        let woken = wake.is_some_and(|at| at <= now);
        if screen != last {
            changes += 1;
            let late = wake.is_none_or(|at| at > now + STEP_PAST);
            assert!(!late, "changed at {now:?} while resting until {wake:?}");
        } else if woken {
            idle += 1;
        }
        if woken || screen != last {
            wake = rig.music.due_in().map(|d| now + d);
        }
        last = screen;
    }
    (changes, idle)
}

#[test]
fn resting_until_the_next_change_is_never_late_on_a_screen_that_follows_the_song() {
    // A song: its time and bar.
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
    rig.heard(10.3, 240.0);
    let (changes, idle) = rests_never_late(&mut rig, 5000, 1.0);
    eprintln!("song: {changes} changes, {idle} idle wakes");
    assert!(
        changes > 20 && idle <= changes / 4,
        "{changes} changes, {idle} idle"
    );
    // Paused, the sleep timer counting down in the corner.
    rig.menu("Sleep Timer");
    rig.keys(&[Button::Right, Button::Right, Button::B, Button::A]);
    assert!(!rig.now_playing().status.playing);
    let (changes, _) = rests_never_late(&mut rig, 3000, 1.0);
    assert!(changes >= 2, "the countdown: {changes}");
    // The lyrics, line by line, and followed again a while after scrolling by hand.
    let lrc = rig._card.path().join("Music/X/A/01 one.lrc");
    std::fs::write(
        &lrc,
        "[00:19.25]One\n[00:20.10]Two\n[00:21.70]Three\n[00:30.00]Four\n",
    )
    .unwrap();
    let t = rig.music.player.current().unwrap();
    let read = crate::lyrics::read(&rig.music.library.tracks[t].path);
    let generation = rig.music.generation;
    rig.music
        .update(Msg::Worker(generation, Done::Lyrics(t, read)));
    rig.keys(&[Button::A]);
    rig.music.browser.push(View::Lyrics, 0);
    rig.heard(19.0, 240.0);
    let (changes, _) = rests_never_late(&mut rig, 3000, 1.0);
    assert!(changes >= 3, "line by line: {changes}");
    rig.keys(&[Button::Down]);
    let (changes, _) = rests_never_late(&mut rig, 5000, 1.0);
    assert!(changes >= 1, "followed again: {changes}");
    // A book at 1.5 times, in a chapter: the chapter's time and bar, the time left, its end.
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(100.0, 600.0);
    let part = rig.music.player.current().unwrap();
    let chapter = |start: f64, title: &str| crate::books::chapters::Chapter {
        start,
        title: title.into(),
    };
    let chapters = vec![chapter(0.0, "One"), chapter(102.7, "Two")];
    rig.music.player.set_chapters(part, chapters);
    rig.music.player.set_speed(1.5);
    let (changes, idle) = rests_never_late(&mut rig, 5000, 1.5);
    eprintln!("book: {changes} changes, {idle} idle wakes");
    assert!(
        changes > 20 && idle <= changes / 4,
        "{changes} changes, {idle} idle"
    );
}

#[test]
fn work_budget_a_screen_changes_only_as_often_as_what_it_shows_can_be_seen_to() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
    assert!(rig.now_playing().status.playing);
    rig.heard(0.0, 240.0);
    // Playing a four-minute song: the time each second and the bar each thousandth, not every
    // frame. The numbers are printed for comparing builds (`task perf:ab`).
    let playing = seen_changes(&mut rig, 10);
    eprintln!("budget: Now Playing, playing 10 s: {playing} changes");
    assert!(playing <= 10 + 42, "{playing} changes in 10 s");
    rig.keys(&[Button::A]);
    assert!(!rig.now_playing().status.playing);
    let paused = seen_changes(&mut rig, 10);
    eprintln!("budget: Now Playing, paused 10 s: {paused} changes");
    assert_eq!(paused, 0, "paused, nothing moves");
    rig.keys(&[Button::B]);
    let list = seen_changes(&mut rig, 10);
    eprintln!("budget: a list, still 10 s: {list} changes");
    assert_eq!(list, 0, "a list left alone holds still");
}

#[test]
fn a_book_never_repeats_but_goes_on_through_its_parts_and_ends() {
    for repeat in [Repeat::All, Repeat::One] {
        let mut rig = Rig::new(&BOOK);
        rig.music.player.set_repeat(repeat);
        let play = rig.music.play_book(BookId(0), false);
        rig.music.send(play);
        rig.frames(FRAME);
        let parts = rig.music.library.albums[rig.music.library.books[0].album]
            .tracks
            .clone();
        let played = |told: Vec<Command>| -> Vec<PathBuf> {
            let paths = told.into_iter().filter_map(|c| match c {
                Command::Play { path, .. } => Some(path),
                _ => None,
            });
            paths.collect()
        };
        rig.told();
        rig.ends();
        let second = rig.music.library.tracks[parts[1]].path.clone();
        assert_eq!(
            played(rig.told()),
            [second],
            "{repeat:?}: on to its next part"
        );
        rig.ends();
        assert_eq!(played(rig.told()), [] as [PathBuf; 0], "{repeat:?}: ended");
    }
}

#[test]
fn a_book_s_chapters_listed_are_read_for_all_its_parts_and_open_where_it_was_left() {
    let mut rig = Rig::new(&[
        "Audiobooks/Wilde - Tales/01 one.m4b",
        "Audiobooks/Wilde - Tales/02 two.m4b",
    ]);
    let music = &mut rig.music;
    let book = BookId(0);
    let parts = music.library.albums[music.library.books[0].album]
        .tracks
        .clone();
    let left = crate::books::shelf::Place {
        part: 1,
        pos: 70.0,
        when: 1,
        ..Default::default()
    };
    music.books.shelf.set_place(&music.library, book, left);
    music.browser.push(View::BookList(crate::BookList::All), 0);
    music.act(Press::Tap(Key::X));
    music.read_chapters();
    assert_eq!(jobs(music, chapters_of), parts, "each part's, none playing");
    music.out.clear();
    let chapters = |titles: [&str; 2]| {
        let at = [0.0, 60.0].into_iter().zip(titles);
        at.map(|(start, title)| crate::books::chapters::Chapter {
            start,
            title: title.into(),
        })
        .collect()
    };
    // The cursor waits on the part it was left in, and follows it as its chapters come.
    music.chapters_read(parts[0], chapters(["One", "Two"]));
    assert_eq!(
        music.browser.at(),
        2,
        "the second part, its chapters not read yet"
    );
    music.chapters_read(parts[1], chapters(["Three", "Four"]));
    assert_eq!(music.browser.at(), 3, "Four, where it was left");
    rig.keys(&[Button::Up, Button::A]);
    let played: Vec<_> = rig
        .told()
        .into_iter()
        .filter(|c| matches!(c, Command::Play { .. }))
        .collect();
    let path = rig.music.library.tracks[parts[1]].path.clone();
    assert_eq!(
        played,
        [Command::Play {
            path,
            at: 0.0,
            speed: 1.0
        }],
        "Three, from its start"
    );
}

#[test]
fn now_playing_says_whether_a_book_plays_for_the_square_with_no_picture() {
    let mut rig = Rig::new(&BOOK);
    let play = rig.music.play_book(BookId(0), false);
    rig.music.send(play);
    rig.music.browser.push(View::NowPlaying, 0);
    rig.frames(FRAME);
    assert!(rig.now_playing().book, "a book");
    let song = rig.music.library.tracks_by_title.clone();
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(song, 0, None));
    rig.music.leave_book();
    rig.music.send(play);
    rig.frames(FRAME);
    assert!(!rig.now_playing().book, "a song");
}

#[test]
fn the_lyrics_are_headed_by_the_song_and_who_sings_it() {
    let mut rig = Rig::new(&["Music/Wilde/A/01 one.mp3"]);
    let lrc = rig._card.path().join("Music/Wilde/A/01 one.lrc");
    std::fs::write(lrc, "[00:01.00]Hello").unwrap();
    let tracks = rig.music.library.tracks_by_title.clone();
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(tracks, 0, None));
    rig.music.send(play);
    rig.music.browser.push(View::Lyrics, 0);
    rig.wait(500);
    assert_eq!(rig.screen().title, "one · Wilde");
}

#[test]
fn the_sleep_timer_counts_down_on_now_playing_and_a_list_keeps_its_count() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    rig.menu("Sleep Timer");
    rig.keys(&[Button::Right, Button::Right, Button::B]);
    rig.hold(Button::B, 600);
    rig.library("Artists");
    rig.keys(&[Button::A]);
    assert_eq!(rig.screen().corner, "1", "its count, as without the timer");
    assert_eq!(
        rig.music.due_in(),
        None,
        "not woken for a countdown it hides"
    );
    rig.keys(&[Button::Select]);
    let corner = rig.screen().corner;
    assert!(corner.starts_with("Sleep"), "on Now Playing: {corner}");
    let due = rig.music.due_in().expect("woken for the countdown");
    assert!(due <= Duration::from_millis(1001), "{due:?}");
}

#[test]
fn help_is_one_page_of_tips_then_the_keys_the_hints_leave_out() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.to("Settings");
    rig.keys(&[Button::A]);
    // Last come Integrations, Help and Advanced: Up from the first goes round to them.
    rig.keys(&[Button::Up, Button::Up]);
    rig.to(text::HELP);
    rig.keys(&[Button::A]);
    let Body::List(page) = rig.screen().body else {
        panic!("a list to read");
    };
    assert_eq!(rig.screen().title, text::HELP);
    assert!(page.readonly, "read, not picked from");
    assert_eq!(page.rows[0], text::TIPS[0], "the tips first");
    assert!(page.rows[0].cased, "a tip a sentence, as written");
    let key = page
        .rows
        .iter()
        .find(|r| !r.after.is_empty())
        .expect("a key");
    assert!(!key.cased, "a key's name in capitals, as on the device");
    // Then each place by name, dim as a heading, and its keys with what they do beside.
    let (place, keys) = text::KEYS[0];
    let at = text::TIPS.len();
    assert_eq!(page.rows[at].before, place);
    assert!(
        !page.rows[at].faint,
        "dim, not faint as what cannot be picked"
    );
    let key = &page.rows[at + 1];
    assert_eq!((key.text.as_str(), key.after.as_str()), keys[0]);
    rig.hold(Button::Down, 8000);
    assert_eq!(
        rig.music.browser.at(),
        text::help_len() - 1,
        "to the last, and no further"
    );
    rig.keys(&[Button::B]);
    assert_eq!(rig.screen().title, "Settings");
    // Advanced, beside it, holds the scrobbler log first.
    rig.to(text::ADVANCED);
    rig.keys(&[Button::A]);
    assert_eq!(rig.rows()[0], "Scrobbler Log");
}

#[test]
fn a_song_picked_from_a_list_lets_the_last_picture_go_and_one_skipped_to_fades_from_it() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/B/01 two.mp3"]);
    let [a, b] = [0, 1].map(|i| rig.music.library.albums[AlbumId(i)].tracks[0]);
    let picture = |album| crate::library::covers::Art {
        album,
        w: 1,
        h: 1,
        rgba: vec![0; 4],
    };
    let play = |rig: &mut Rig, t: TrackId| {
        let play = rig
            .music
            .player
            .play(&rig.music.library, Queue::new(vec![t], 0, None));
        rig.music.send(play);
        rig.frames(FRAME);
    };
    // On Now Playing, A's picture up, a skip to B keeps it there to fade from.
    rig.music.browser.push(View::NowPlaying, 0);
    play(&mut rig, a);
    rig.music.pictures.art = Some(picture(rig.music.library.tracks[a].album));
    play(&mut rig, b);
    assert!(rig.music.pictures.art.is_some(), "the last, to fade from");
    // From a list, the picture is let go: Now Playing opens on the new one.
    rig.music.browser.pop();
    rig.music.show_art(false);
    rig.music.pictures.art = Some(picture(rig.music.library.tracks[b].album));
    play(&mut rig, a);
    assert!(rig.music.pictures.art.is_none(), "not the last");
}

#[test]
fn the_next_song_s_picture_is_read_ahead_so_a_skip_to_it_shows_it_at_once() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/B/01 two.mp3"]);
    let [a, b] = [0, 1].map(|i| rig.music.library.albums[AlbumId(i)].tracks[0]);
    let album_b = rig.music.library.tracks[b].album;
    let loads = |music: &Music| -> Vec<AlbumId> {
        let asked = music.out.iter().filter_map(|e| match e {
            Effect::Work(_, Job::Load { album, .. }) => Some(*album),
            _ => None,
        });
        asked.collect()
    };
    rig.music.browser.push(View::NowPlaying, 0);
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(vec![a, b], 0, None));
    rig.music.send(play);
    rig.music.stamp();
    rig.music.show_art(false);
    assert!(loads(&rig.music).contains(&album_b), "the next's, ahead");
    rig.music.out.clear();
    let art = crate::library::covers::Art {
        album: album_b,
        w: 1,
        h: 1,
        rgba: vec![0; 4],
    };
    rig.music.on_art(Done::Loaded(album_b, Some(art)));
    let skip = rig.music.player.skip(&rig.music.library, 1);
    rig.music.send(skip);
    rig.music.show_art(false);
    let shown = rig.music.pictures.art.as_ref().map(|a| a.album);
    assert_eq!(shown, Some(album_b), "there at once");
    assert!(!loads(&rig.music).contains(&album_b), "not read again");
}

#[test]
fn the_shoulders_jump_a_letter_in_a_list_by_name_turn_a_page_in_another_and_do_nothing_on_a_menu() {
    let files: Vec<String> = (1..=12)
        .map(|i| format!("Music/Abba/A/{i:02} t{i}.mp3"))
        .collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut rig = Rig::new(&files);
    let songs = rig.music.library.tracks_by_title.clone();
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(songs, 0, None));
    rig.music.send(play);
    rig.frames(FRAME);
    rig.told();
    let skipped = |rig: &mut Rig| rig.told().iter().any(|c| matches!(c, Command::Play { .. }));
    // The Music menu, and a setting, whose Left and Right change it: nothing.
    rig.keys(&[Button::R1]);
    assert!(!skipped(&mut rig), "not on the menu");
    assert_eq!(rig.music.browser.at(), 0);
    rig.to("Settings");
    rig.keys(&[Button::A, Button::Down]);
    let was = rig.music.settings;
    rig.keys(&[Button::R1, Button::L1]);
    assert!(
        !skipped(&mut rig) && rig.music.settings == was,
        "nor on a setting"
    );
    // An album's songs go by number: a page on.
    rig.hold(Button::B, 600);
    rig.library("Albums");
    rig.keys(&[Button::A, Button::Down, Button::A, Button::R1]);
    assert!(!skipped(&mut rig));
    assert_eq!(
        rig.music.browser.at(),
        crate::browse::LIST_ROWS,
        "a page on"
    );
    // SELECT with one skips from anywhere.
    rig.down(Button::Select);
    rig.keys(&[Button::R1]);
    rig.up(Button::Select);
    assert!(skipped(&mut rig), "SELECT and R1 skip");
}

#[test]
fn detailed_log_is_kept_and_turns_the_log_s_detail_on_and_off() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    let log = crate::browse::rows::Row::Setting(crate::settings::Setting::Log);
    let at = rows(&rig.music.context(), View::Advanced)
        .iter()
        .position(|&r| r == log);
    rig.music.browser.push(View::Advanced, at.unwrap());
    rig.music.act(Press::Tap(Key::A));
    assert!(rig.music.settings.log);
    let asked = rig.music.out.iter().filter_map(|e| match e {
        Effect::Log(on) => Some(*on),
        Effect::Setting("log", value) => Some(value == "on"),
        _ => None,
    });
    assert_eq!(
        asked.collect::<Vec<_>>(),
        [true, true],
        "kept, and told the logger"
    );
}

impl Rig {
    /// From the Music menu, Settings › Music › Equalizer.
    fn equalizer(&mut self) {
        for row in ["Settings", text::MUSIC_SETTINGS, text::EQUALIZER] {
            self.to(row);
            self.keys(&[Button::A]);
        }
        assert_eq!(self.music.browser.view(), View::Equalizer);
    }

    /// The gains the player was last told to play through, since last asked; none for none.
    fn told_eq(&mut self) -> Option<Vec<f32>> {
        let mut told = self.told().into_iter().filter_map(|c| match c {
            Command::Equalizer(bands) => Some(bands.iter().map(|b| b.db).collect()),
            _ => None,
        });
        told.next_back()
    }

    /// The value beside the row named `row` on the list shown.
    fn screen_value(&self, row: &str) -> String {
        let Body::List(list) = self.screen().body else {
            panic!("no list for {row}");
        };
        let at = list
            .rows
            .iter()
            .position(|r| r.text == row)
            .expect("the row");
        list.values[at].clone().unwrap_or_default()
    }

    fn hint_labels(&self) -> Vec<String> {
        let hints = text::legend(self.screen().legend);
        hints.iter().map(text::Hint::label).collect()
    }
}

#[test]
fn the_equalizer_chooses_a_preset_with_a_kept_by_name_and_plays_through_it() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.equalizer();
    let rows = rig.rows();
    assert_eq!(rows[0], "Off");
    assert_eq!(rows.last().map(String::as_str), Some(text::NEW_PRESET));
    let first = rig.music.equalizer.get(1).unwrap().clone();
    rig.to(&first.name);
    assert!(
        !rig.hint_labels().iter().any(|h| h.contains('(')),
        "slot's own cannot be deleted: {:?}",
        rig.hint_labels()
    );
    rig.told();
    rig.keys(&[Button::A]);
    assert_eq!(rig.told_eq(), Some(first.gains.to_vec()), "heard at once");
    let kept = rig
        .music
        .out
        .iter()
        .any(|e| matches!(e, Effect::Setting("eq", n) if *n == first.name));
    assert!(kept || rig.music.settings.eq == 1, "kept by name");
    let Body::List(list) = rig.screen().body else {
        panic!("the presets");
    };
    assert!(list.rows[1].playing, "marked as what plays");
    rig.keys(&[Button::B]);
    assert_eq!(
        rig.screen_value(text::EQUALIZER),
        first.name,
        "named on Music's settings"
    );
}

#[test]
fn a_new_preset_is_heard_as_it_is_set_kept_with_y_and_deleted_with_x_held() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.equalizer();
    let slot_s = rig.music.equalizer.len();
    rig.to(text::NEW_PRESET);
    rig.keys(&[Button::A]);
    assert_eq!(rig.music.browser.view(), View::NewPreset);
    assert_eq!(rig.rows().len(), crate::store::equalizer::BANDS);
    rig.told();
    rig.keys(&[Button::Right, Button::Right, Button::Right]);
    assert_eq!(
        rig.told_eq(),
        Some(vec![3.0, 0.0, 0.0, 0.0, 0.0]),
        "heard as it is set"
    );
    // B leaves it unkept, and what was chosen, off, plays again.
    rig.keys(&[Button::B]);
    assert_eq!(rig.told_eq(), Some(Vec::new()));
    assert_eq!(rig.music.equalizer.len(), slot_s);
    // Set again, from what is chosen, and kept with Y.
    rig.keys(&[Button::A]);
    rig.keys(&[Button::Down, Button::Left, Button::Left, Button::Y]);
    assert_eq!(
        rig.music.browser.view(),
        View::Equalizer,
        "back on the presets"
    );
    let own = text::own_preset(1);
    assert_eq!(
        rig.music.equalizer.name(rig.music.settings.eq),
        Some(own.as_str()),
        "chosen"
    );
    assert_eq!(rig.screen().corner, text::notice(&Notice::Saved));
    let Body::List(list) = rig.screen().body else {
        panic!("the presets");
    };
    assert_eq!(list.rows[list.selected].text, own, "at it");
    rig.music.flush();
    let file = rig._card.path().join(crate::store::equalizer::EQ_FILE);
    let kept = std::fs::read_to_string(&file).unwrap();
    assert!(kept.contains(&format!("{own} = 0, -2, 0, 0, 0")), "{kept}");
    // One's own deletes with X held, as the hint says.
    assert!(
        rig.hint_labels().contains(&"(Delete)".to_string()),
        "{:?}",
        rig.hint_labels()
    );
    rig.keys(&[Button::X]);
    assert_eq!(rig.music.equalizer.len(), slot_s + 1, "tapped, nothing");
    rig.hold(Button::X, 700);
    assert_eq!(rig.music.equalizer.len(), slot_s, "held, deleted");
    assert_eq!(rig.music.settings.eq, 0, "off, as it was chosen");
    rig.music.flush();
    assert!(!std::fs::read_to_string(&file).unwrap().contains(&own));
}

#[test]
fn with_the_scrobbler_log_on_a_song_heard_half_way_is_written_for_an_uploader() {
    for on in [false, true] {
        let mut rig = Rig::new(&["Music/Abba/Gold/03 sos.mp3"]);
        rig.music.settings.scrobbler = on;
        let tracks = rig.music.library.tracks_by_title.clone();
        let play = rig
            .music
            .player
            .play(&rig.music.library, Queue::new(tracks, 0, None));
        rig.music.send(play);
        rig.frames(FRAME);
        rig.heard(30.0, LEN);
        rig.heard(50.0, LEN);
        rig.heard(60.0, LEN);
        rig.music.flush();
        let log = std::fs::read_to_string(rig._card.path().join(".scrobbler.log"));
        if !on {
            assert!(log.is_err(), "off: nothing written");
            continue;
        }
        let log = log.unwrap();
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(
            &lines[..3],
            ["#AUDIOSCROBBLER/1.1", "#TZ/UTC", "#CLIENT/slot"]
        );
        assert_eq!(lines.len(), 4, "once: {log}");
        let fields: Vec<&str> = lines[3].split('\t').collect();
        assert_eq!(&fields[..6], ["Abba", "Gold", "sos", "03", "100", "L"]);
    }
}

#[test]
fn a_song_skipped_to_while_paused_that_will_not_open_says_so_and_the_next_waits() {
    let mut rig = Rig::new(&[
        "Music/X/A/01 a.mp3",
        "Music/X/A/02 b.mp3",
        "Music/X/A/03 c.mp3",
    ]);
    rig.library("Songs");
    rig.keys(&[Button::A]);
    rig.to("a");
    rig.keys(&[Button::A, Button::A]);
    assert_eq!(rig.music.player.state(), PlayState::Paused);
    rig.keys(&[Button::R1]);
    rig.told();
    rig.says(Event::Error("b: cannot open".into()));
    assert_eq!(rig.screen().corner, text::notice(&Notice::Unplayable));
    assert_eq!(rig.now_playing().title, "c");
    assert!(matches!(rig.told()[..], [Command::Cue { .. }, ..]));
}

#[test]
fn a_new_library_keeps_the_lists_open_and_their_cursors_but_not_what_it_renumbers() {
    let mut rig = Rig::new(&[
        "Music/Abba/A/01 one.mp3",
        "Music/Cure/B/01 two.mp3",
        "Music/Dido/C/01 three.mp3",
    ]);
    rig.library("Artists");
    rig.keys(&[Button::A]);
    rig.to("Cure");
    // The card's own reading comes in behind the cache's, an artist added ahead of the cursor.
    let card = rig.music.root.clone();
    std::fs::create_dir_all(card.join("Music/Blur/D")).unwrap();
    std::fs::write(card.join("Music/Blur/D/01 four.mp3"), b"").unwrap();
    rig.music.update(Msg::Library(Found::Fresh(
        Box::new(Library::scan(&card)),
        Vec::new(),
    )));
    rig.frames(FRAME);
    assert_eq!(rig.music.browser.view(), View::Artists);
    assert_eq!(
        rig.music
            .browser
            .row(&rig.music.context())
            .map(|r| crate::browse::rows::label(&rig.music.context(), View::Artists, r).text),
        Some("Cure".into())
    );
    // An artist's albums go by its number: back to the list it was opened from.
    rig.keys(&[Button::A]);
    assert!(matches!(rig.music.browser.view(), View::Artist(_)));
    rig.music.update(Msg::Library(Found::Fresh(
        Box::new(Library::scan(&card)),
        Vec::new(),
    )));
    rig.frames(FRAME);
    assert_eq!(rig.music.browser.view(), View::Artists);
}

#[test]
fn select_held_long_is_still_a_tap_to_now_playing_as_it_is_let_go() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.keys(&[Button::A]);
    rig.hold(Button::B, 600);
    rig.hold(Button::Select, 1500);
    assert_eq!(rig.music.browser.view(), View::NowPlaying);
}

#[test]
fn an_empty_card_sets_back_what_leads_nowhere_and_says_where_to_put_songs_and_books() {
    let mut rig = Rig::new(&[]);
    let Body::List(list) = rig.screen().body else {
        panic!("the Music menu");
    };
    let faint: Vec<(&str, bool)> = list
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.faint))
        .collect();
    assert_eq!(
        faint,
        [
            (text::SHUFFLE_LIBRARY, true),
            (text::QUEUE, true),
            (text::LIBRARY, true),
            ("Playlists", true),
            (text::BOOKS, true),
            ("Settings", false),
        ]
    );
    assert_eq!(rig.screen().corner, text::NOTHING_YET);
    // Shuffle Library waits for songs.
    rig.keys(&[Button::A]);
    assert_eq!(rig.music.browser.view(), View::Main);
    rig.to(text::LIBRARY);
    rig.keys(&[Button::A]);
    assert_eq!(rig.screen().body, Body::Message(text::no_music("Music")));
    rig.keys(&[Button::B]);
    rig.to(text::BOOKS);
    rig.keys(&[Button::A]);
    assert_eq!(
        rig.screen().body,
        Body::Message(text::no_books("Audiobooks"))
    );
    // A book alone: the books lit, the music still set back, and the corner quiet.
    let mut rig = Rig::new(&["Audiobooks/Kipling - Just So/01 whale.mp3"]);
    let Body::List(list) = rig.screen().body else {
        panic!("the Music menu");
    };
    assert!(
        list.rows[2].faint && !list.rows[4].faint,
        "Library set back, Audiobooks lit"
    );
    assert_eq!(rig.screen().corner, "");
    rig.to(text::LIBRARY);
    rig.keys(&[Button::A]);
    assert_eq!(rig.screen().body, Body::Message(text::no_music("Music")));
}

#[test]
fn a_scan_that_dies_before_a_library_comes_is_waited_on_no_more_and_the_queue_file_is_kept() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    let card = rig.music.root.clone();
    let queue = "#EXTM3U\n#SLOT-AT:0\n#SLOT-POS:5.0\nMusic/X/A/01 one.mp3\n";
    std::fs::write(card.join(SavedQueue::FILE), queue).unwrap();
    // A fresh boot whose scan thread ends without a word.
    rig.music.scan.ready = false;
    rig.music.scan.queue_text = Some(queue.into());
    rig.music.player.wait_for_saved();
    let (tx, rx) = std::sync::mpsc::channel::<Found>();
    drop(tx);
    rig.music.scan.rx = Some(rx);
    rig.frames(FRAME);
    assert!(rig.music.scan.ready && !rig.music.scanning());
    assert_ne!(rig.screen().body, Body::Message(text::reading(0, 0)));
    assert_eq!(
        rig.rows()[0],
        text::SHUFFLE_LIBRARY,
        "not waiting on the queue"
    );
    rig.music.flush();
    assert_eq!(
        std::fs::read_to_string(card.join(SavedQueue::FILE)).unwrap(),
        queue
    );
}

#[test]
fn a_write_the_card_refuses_is_said_in_the_corner() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    // Config a file, not a folder: nothing can be written in it, as on a card full or locked.
    let config = rig.music.root.join("Config");
    std::fs::remove_dir_all(&config).ok();
    std::fs::write(&config, b"").unwrap();
    rig.keys(&[Button::Up, Button::A, Button::Down, Button::A]);
    rig.music.flush();
    rig.frames(FRAME);
    assert_eq!(rig.screen().corner, text::notice(&Notice::CantSave));
}

#[test]
fn a_song_s_place_is_saved_now_and_then_as_it_plays_for_a_sudden_power_cut() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.library("Songs");
    rig.keys(&[Button::A, Button::Down, Button::Down, Button::A]);
    assert!(rig.now_playing().status.playing);
    let saved = |rig: &mut Rig| {
        rig.music.flush();
        let text = std::fs::read_to_string(rig.music.root.join(SavedQueue::FILE)).unwrap();
        let pos = text
            .lines()
            .find_map(|l| l.strip_prefix("#SLOT-POS:"))
            .unwrap();
        pos.parse::<f64>().unwrap()
    };
    let play_for = |rig: &mut Rig, secs: u64| {
        for _ in 0..secs * 50 {
            rig.fake.0.borrow_mut().status.at += FRAME.as_secs_f64();
            rig.frames(FRAME);
        }
    };
    play_for(&mut rig, 60);
    assert!(
        saved(&mut rig) < 1.0,
        "a minute in: not yet, the card spared"
    );
    play_for(&mut rig, 60);
    let pos = saved(&mut rig);
    assert!((119.0..=121.0).contains(&pos), "two minutes in: {pos}");
}

#[test]
fn an_album_played_in_order_stays_gapless_and_a_change_of_album_crossfades() {
    let mut rig = Rig::new(&[
        "Music/X/A/01 one.mp3",
        "Music/X/A/02 two.mp3",
        "Music/X/B/01 three.mp3",
    ]);
    rig.music.settings.crossfade = 6;
    let fades = |rig: &mut Rig| -> Vec<f64> {
        let told = rig.told();
        told.iter()
            .filter_map(|c| match c {
                Command::Crossfade(s) => Some(*s),
                _ => None,
            })
            .collect()
    };
    let lib = &rig.music.library;
    let in_order: Vec<TrackId> = lib.albums.iter().flat_map(|a| a.tracks.clone()).collect();
    let play = rig.music.player.play(lib, Queue::new(in_order, 0, None));
    rig.music.send(play);
    rig.frames(FRAME);
    assert_eq!(
        fades(&mut rig),
        [0.0],
        "one into two, on its album: no fade"
    );
    rig.ends();
    assert_eq!(fades(&mut rig), [6.0], "two into another album's: the fade");
    rig.ends();
    assert_eq!(
        fades(&mut rig),
        [] as [f64; 0],
        "the last, with none after: as it was"
    );
    // The other parts of the sound are not told again for it.
    let play = rig.music.player.play(
        &rig.music.library,
        Queue::new(vec![TrackId(0), TrackId(1)], 0, None),
    );
    rig.music.send(play);
    rig.frames(FRAME);
    let told = rig.told();
    assert!(
        !told
            .iter()
            .any(|c| matches!(c, Command::Equalizer(_) | Command::Mono(_))),
        "{told:?}"
    );
}

#[test]
fn the_first_start_opens_on_help_once_and_b_closes_it() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    let card = rig.music.root.clone();
    std::fs::remove_file(card.join("Config/music.txt")).unwrap();
    let start: engine::Start = Box::new(|| Box::new(Fake::default()));
    let mut first = Music::with(&card, (64, 16), start, Box::new(system_now));
    assert_eq!(first.browser.view(), View::Help, "a card never started");
    first.input(Input::Down(Button::B));
    first.input(Input::Up(Button::B));
    assert_eq!(first.browser.view(), View::Main, "B closes it");
    first.flush();
    let start: engine::Start = Box::new(|| Box::new(Fake::default()));
    let again = Music::with(&card, (64, 16), start, Box::new(system_now));
    assert_eq!(again.browser.view(), View::Main, "once only");
    rig.frames(FRAME);
}

#[test]
fn a_list_s_rows_start_with_their_album_s_thumbnail_asked_for_by_page_as_it_changes() {
    let files: Vec<String> = (0..10)
        .map(|i| format!("Music/X/Album {i:02}/01 t.mp3"))
        .collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut rig = Rig::new(&files);
    rig.library("Albums");
    rig.keys(&[Button::A]);
    let thumbs = |music: &Music| -> Vec<AlbumId> {
        let jobs = music.out.iter().filter_map(|e| match e {
            Effect::Work(_, Job::Thumb { album, .. }) => Some(*album),
            _ => None,
        });
        jobs.collect()
    };
    // The first page: Play All, then a page of albums. Its pictures from afresh, to see them
    // asked for.
    rig.music.pictures = Default::default();
    rig.music.frame(Status::default(), Vec::new(), false);
    let asked = thumbs(&rig.music);
    assert_eq!(
        asked.len(),
        crate::browse::RICH_ROWS,
        "the page's albums, once"
    );
    rig.music.out.clear();
    rig.music.frame(Status::default(), Vec::new(), false);
    assert!(thumbs(&rig.music).is_empty(), "not again on the same page");
    let Body::List(list) = rig.screen().body else {
        panic!("a list");
    };
    assert!(
        list.thumbs && list.rows[1].album.is_none(),
        "room kept, none read yet"
    );
    // One read: its row names its album, so the screen changes and is drawn.
    let art = crate::Art {
        album: asked[0],
        w: 2,
        h: 2,
        rgba: vec![0; 16],
    };
    let generation = rig.music.generation;
    rig.music
        .update(Msg::Worker(generation, Done::Thumb(asked[0], Some(art))));
    let Body::List(list) = rig.screen().body else {
        panic!("a list");
    };
    assert_eq!(list.rows[1].album, Some(asked[0]));
    assert_eq!(rig.music.thumbs().len(), 1);
    // The next page lets the first page's go, and asks for its own.
    rig.music.out.clear();
    rig.keys(&[Button::Right]);
    assert!(rig.music.thumbs().is_empty(), "let go");
    // An album's own list numbers its songs: no thumbnails there.
    rig.keys(&[Button::A]);
    let Body::List(list) = rig.screen().body else {
        panic!("a list");
    };
    assert!(!list.thumbs);
}

#[test]
fn open_on_now_playing_opens_music_on_what_plays_and_else_where_it_was_left() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.library("Songs");
    rig.keys(&[Button::A, Button::Down, Button::Down, Button::A]);
    assert!(rig.now_playing().status.playing);
    rig.hold(Button::B, 600);
    rig.to("Settings");
    rig.keys(&[Button::A]);
    // Off, as it comes: where it was left.
    rig.music.open();
    assert_eq!(rig.music.browser.view(), View::Settings);
    rig.music.settings.open_playing = true;
    rig.music.open();
    assert_eq!(rig.music.browser.view(), View::NowPlaying, "on what plays");
    rig.keys(&[Button::B]);
    assert_eq!(
        rig.music.browser.view(),
        View::Main,
        "B back to the Music menu"
    );
    // Paused, nothing plays: where it was left.
    rig.keys(&[Button::Select, Button::A, Button::B]);
    rig.music.open();
    assert_eq!(rig.music.browser.view(), View::Main);
}

#[test]
fn under_the_lid_a_jump_that_lands_in_another_chapter_buzzes_and_one_within_does_not() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    let part = rig.music.player.current().unwrap();
    let chapter = |start: f64| crate::books::chapters::Chapter {
        start,
        title: String::new(),
    };
    rig.music
        .player
        .set_chapters(part, vec![chapter(0.0), chapter(50.0)]);
    rig.music.settings.jump = 30;
    rig.heard(90.0, 100.0);
    rig.music.set_dark(true);
    rig.keys(&[Button::L1]);
    assert_eq!(
        rig.music.rumble(),
        None,
        "90 to 60: the same chapter, no buzz"
    );
    rig.heard(60.0, 100.0);
    rig.keys(&[Button::L1]);
    assert_eq!(
        rig.music.rumble(),
        Some(u16::MAX),
        "60 to 30: the chapter before, a buzz"
    );
}

#[test]
fn a_tick_unseen_is_half_as_long_as_a_lock_s_buzz_so_the_two_are_told_apart() {
    let mut rig = Rig::new(&BOOK);
    let at = |ms: u64, rig: &Rig| rig.music.now.at + Duration::from_millis(ms);
    let lasts = |rig: &mut Rig| {
        let start = rig.music.now.at;
        let mut on = None;
        for ms in (0..1000).step_by(10) {
            let now = start + Duration::from_millis(ms);
            match (rig.music.buttons.pocket.rumble(now), on) {
                (Some(0), Some(from)) => return ms - from,
                (Some(s), None) if s > 0 => on = Some(ms),
                _ => {}
            }
        }
        0
    };
    rig.music.buttons.pocket.tick(at(0, &rig));
    let tick = lasts(&mut rig);
    rig.music.buttons.pocket.toggle(at(0, &rig));
    let lock = lasts(&mut rig);
    assert_eq!(tick, pocket::BUZZ.as_millis() as u64);
    assert_eq!(lock, pocket::LOCK_BUZZ.as_millis() as u64);
}

#[test]
fn the_song_before_s_lyrics_are_read_ahead_too_so_going_back_finds_its_hints_steady() {
    let mut rig = Rig::new(&[
        "Music/X/A/01 one.mp3",
        "Music/X/A/02 two.mp3",
        "Music/X/A/03 three.mp3",
    ]);
    let music = &mut rig.music;
    let tracks = music.library.albums[AlbumId(0)].tracks.clone();
    let play = music
        .player
        .play(&music.library, Queue::new(tracks.clone(), 1, None));
    music.send(play);
    music.browser.push(View::NowPlaying, 0);
    music.out.clear();
    music.read_lyrics();
    let mut asked = jobs(music, lyrics_of);
    asked.sort();
    assert_eq!(
        asked, tracks,
        "the song playing, the next and the one before"
    );
}

#[test]
fn update_library_goes_back_to_the_music_menu_where_its_bar_fills() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3"]);
    rig.to("Settings");
    rig.keys(&[Button::A]);
    rig.to("Update Library");
    rig.keys(&[Button::A]);
    assert_eq!(rig.music.browser.view(), View::Main);
    assert!(rig.music.scanning());
    // Halfway through what it found to read, as the test has it rather than the scan.
    let progress = std::sync::Arc::new(crate::Progress::default());
    progress.set(1, 2);
    rig.music.scan.progress = progress;
    let Body::List(list) = rig.screen().body else {
        panic!("the Music menu");
    };
    assert_eq!(list.progress, Some(0.5));
}

#[test]
fn the_queue_on_the_music_menu_opens_on_the_song_playing() {
    let mut rig = Rig::new(&[
        "Music/X/A/01 a.mp3",
        "Music/X/A/02 b.mp3",
        "Music/X/A/03 c.mp3",
    ]);
    let tracks = rig.music.library.albums[AlbumId(0)].tracks.clone();
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(tracks, 1, None));
    rig.music.send(play);
    rig.frames(FRAME);
    rig.to(text::QUEUE);
    rig.keys(&[Button::A]);
    assert_eq!(rig.music.browser.view(), View::Queue);
    assert_eq!(rig.music.browser.at(), 1, "on b, playing");
    rig.keys(&[Button::B]);
    assert_eq!(rig.music.browser.view(), View::Main);
}

#[test]
fn a_start_menu_hints_a_change_only_on_a_row_that_changes() {
    let mut rig = Rig::new(&["Music/X/A/01 a.mp3", "Music/X/A/02 b.mp3"]);
    rig.library("Songs");
    rig.keys(&[Button::A, Button::Down, Button::Down, Button::A]);
    rig.keys(&[Button::Start]);
    let changes = |rig: &Rig| {
        text::legend(rig.screen().legend)
            .iter()
            .any(|h| h.key == text::ARROWS)
    };
    rig.to("Song Info");
    assert!(!changes(&rig), "Song Info opens; nothing to change");
    rig.to("Sleep Timer");
    assert!(changes(&rig), "the sleep timer changes");
}

#[test]
fn the_visualizer_asks_for_the_levels_only_while_they_are_seen_and_shows_them_as_bars() {
    use slot_player::BANDS;
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    let asked = |rig: &mut Rig, on| rig.told().contains(&Command::Spectrum(on));
    assert!(!asked(&mut rig, true), "off, never asked");
    rig.music.settings.visualizer = true;
    rig.frames(FRAME);
    assert!(asked(&mut rig, true), "on Now Playing, asked");
    assert_eq!(rig.now_playing().bars, None, "none given yet");
    rig.fake.0.borrow_mut().status.spectrum = Some([255; BANDS]);
    rig.frames(FRAME);
    assert_eq!(
        rig.now_playing().bars,
        Some([31; BANDS]),
        "the loudest, the tallest"
    );
    let due = rig.music.due_in().expect("woken for the bars");
    assert!(due <= Duration::from_millis(35), "as a film moves: {due:?}");
    rig.keys(&[Button::B]);
    assert!(asked(&mut rig, false), "away from Now Playing, no more");
}

#[test]
fn set_to_play_locked_the_lid_shut_locks_the_keys_and_opened_gives_them_back() {
    use crate::settings::ScreenOff;
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::A]);
    rig.music.settings.screen_off = ScreenOff::Play;
    rig.music.set_dark(true);
    assert!(
        !rig.music.buttons.pocket.locked,
        "plays on, the shoulders still its"
    );
    rig.music.set_dark(false);
    rig.music.settings.screen_off = ScreenOff::PlayLocked;
    rig.music.set_dark(true);
    assert!(
        rig.music.buttons.pocket.locked,
        "shut, locked, with no combo"
    );
    rig.told();
    rig.keys(&[Button::R1, Button::R2]);
    assert_eq!(rig.told(), [], "nothing changes in a pocket");
    rig.music.set_dark(false);
    assert!(
        !rig.music.buttons.pocket.locked,
        "opened, the keys are back"
    );
}

#[test]
fn the_screen_going_off_pauses_unless_set_to_play_on() {
    use crate::settings::ScreenOff;
    for (choice, plays) in [
        (ScreenOff::Pause, false),
        (ScreenOff::Play, true),
        (ScreenOff::PlayLocked, true),
    ] {
        let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
        rig.music.settings.screen_off = choice;
        rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
        assert!(rig.now_playing().status.playing, "{choice:?}");
        rig.told();
        rig.music.set_dark(true);
        let told = rig.told();
        assert_eq!(
            told.contains(&Command::Pause),
            !plays,
            "{choice:?}: {told:?}"
        );
        rig.music.set_dark(false);
        assert_eq!(rig.now_playing().status.playing, plays, "{choice:?}");
    }
}

#[test]
fn timed_lyrics_say_synced_and_both_kinds_show_how_far_into_the_song_seen_as_it_moves() {
    for (lrc, synced) in [
        ("[00:01.00]Hello\n[00:30.00]Again", true),
        ("Hello\nAgain", false),
    ] {
        let mut rig = Rig::new(&["Music/Wilde/A/01 one.mp3"]);
        std::fs::write(rig._card.path().join("Music/Wilde/A/01 one.lrc"), lrc).unwrap();
        rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
        rig.heard(60.0, 240.0);
        rig.music.browser.push(View::Lyrics, 0);
        // The worker's answer, as it reads them, rather than waited on.
        let t = rig.music.player.current().unwrap();
        let read = crate::lyrics::read(&rig.music.library.tracks[t].path);
        let generation = rig.music.generation;
        rig.music
            .update(Msg::Worker(generation, Done::Lyrics(t, read)));
        rig.frames(FRAME);
        let screen = rig.screen();
        let corner = if synced { text::SYNCED } else { "" };
        assert_eq!(screen.corner, corner, "{lrc}");
        let Body::Text(body) = screen.body else {
            panic!("the lyrics");
        };
        let done = body.progress.expect("the song's bar");
        assert!((done - 0.25).abs() < 0.01, "a quarter in: {done}");
        assert_eq!(body.time, Some(text::elapsed(60.0, 240.0)), "and its time");
        // The bar moves a thousandth at a time and the time a second, the screen with them.
        let seen = seen_changes(&mut rig, 10);
        assert!(
            seen > 0 && seen <= 42 + 10 + 2,
            "{lrc}: {seen} changes in 10 s"
        );
    }
}

#[test]
fn the_queue_says_how_long_it_has_left_beside_where_the_cursor_is() {
    let mut rig = Rig::new(&[
        "Music/X/A/01 a.mp3",
        "Music/X/A/02 b.mp3",
        "Music/X/A/03 c.mp3",
    ]);
    let tracks = rig.music.library.albums[AlbumId(0)].tracks.clone();
    let play = rig
        .music
        .player
        .play(&rig.music.library, Queue::new(tracks, 1, None));
    rig.music.send(play);
    rig.frames(FRAME);
    rig.heard(60.0, 240.0);
    rig.to(text::QUEUE);
    rig.keys(&[Button::A]);
    // The songs after are of no known length here: the rest of the one playing.
    assert_eq!(
        rig.screen().corner,
        text::queue_corner(&text::of(2, 3), 180.0)
    );
    let due = rig.music.due_in().expect("woken for the next minute");
    assert!(due <= Duration::from_millis(60_001), "{due:?}");
}

#[test]
fn the_music_menu_leads_with_what_plays_on_two_lines_beside_its_picture() {
    let mut rig = Rig::new(&["Music/Abba/Gold/01 one.mp3", "Music/Abba/Gold/02 two.mp3"]);
    let menu = |rig: &Rig| match rig.screen().body {
        Body::List(list) => list,
        body => panic!("the Music menu: {body:?}"),
    };
    let first = menu(&rig);
    assert!(
        !first.lead,
        "Shuffle Library, with nothing to resume, is one line"
    );
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
    rig.hold(Button::B, 600);
    let playing = menu(&rig);
    assert!(playing.lead);
    let row = &playing.rows[0];
    assert_eq!(
        (row.text.as_str(), row.after.as_str()),
        (text::NOW_PLAYING, "one · Abba")
    );
    let album = rig.music.library.tracks[rig.music.player.current().unwrap()].album;
    let asked = rig.music.pictures.wanted.contains(&album);
    assert!(asked, "its album's thumbnail asked for");
    rig.hold(Button::Start, 600);
    assert_eq!(
        menu(&rig).rows[0].text,
        text::RESUME,
        "paused, Resume, still leading"
    );
    assert!(menu(&rig).lead);
}

#[test]
fn what_plays_is_marked_playing_or_paused_in_the_queue_and_on_the_music_menu() {
    let mut rig = Rig::new(&["Music/Abba/Gold/01 one.mp3", "Music/Abba/Gold/02 two.mp3"]);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
    let list = |rig: &Rig| match rig.screen().body {
        Body::List(list) => list,
        body => panic!("a list: {body:?}"),
    };
    rig.keys(&[Button::Select]);
    assert_eq!(rig.music.browser.view(), View::Queue);
    let queue = list(&rig);
    assert!(queue.rows[0].playing && !queue.paused, "marked, playing");
    rig.hold(Button::Start, 600);
    assert!(list(&rig).paused, "paused, and its mark says so");
    rig.hold(Button::B, 600);
    let menu = list(&rig);
    assert!(
        menu.lead && menu.rows[0].playing && menu.paused,
        "the same on the Music menu"
    );
}

#[test]
fn lists_with_no_songs_yet_say_so_rather_than_offer_play_all() {
    let mut rig = Rig::new(&["Music/Abba/Gold/01 one.mp3"]);
    rig.to("Playlists");
    rig.keys(&[Button::A]);
    for (row, says) in [
        (text::FAVORITES, text::NO_FAVORITES),
        (text::RECENTLY_PLAYED, text::NOTHING_PLAYED),
    ] {
        rig.to(row);
        rig.keys(&[Button::A]);
        let screen = rig.screen();
        assert_eq!(
            (screen.title.as_str(), screen.body),
            (row, Body::Message(says.into()))
        );
        rig.keys(&[Button::B]);
    }
}

#[test]
fn the_book_playing_is_marked_in_its_list_as_a_song_playing_is() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.keys(&[Button::B]);
    let Body::List(list) = rig.screen().body else {
        panic!("All Books");
    };
    assert!(list.rows[0].playing, "{:?}", list.rows[0]);
}
