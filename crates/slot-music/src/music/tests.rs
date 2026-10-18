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
use crate::browse::View;
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
        let mut music = Music::with(card.path(), 64, start, clock);
        // The library as the scan finds it, without waiting on its thread.
        let lib = Library::scan(card.path());
        music.update(Msg::Library(Found::Fresh(lib, Vec::new())));
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
fn a_book_s_now_playing_shows_its_part_place_and_time_left() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(30.0, 60.0);
    let np = rig.now_playing();
    assert_eq!(np.title, "whale", "the part, a chapter of its own");
    assert_eq!((np.by.as_str(), np.from.as_str()), ("Kipling", "Just So"));
    assert_eq!(np.place, text::of(1, 2));
    assert_eq!(np.time, text::elapsed(30.0, 60.0));
    assert_eq!(np.progress, Some(0.5));
    // The parts' lengths not known, the time left is this part's.
    assert_eq!(np.next, Some(text::book_left(1.0, None, 30.0)));
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
    rig.keys(&[Button::Up]);
    assert_eq!(
        rig.screen().corner,
        text::notice(&Notice::Repeat(Repeat::All))
    );
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
fn a_book_playing_through_update_library_plays_on_as_music_does() {
    let mut rig = Rig::new(&BOOK);
    rig.play_book();
    rig.heard(1.0, 60.0);
    rig.told();
    rig.music.rescan(false);
    let lib = Library::scan(&rig.music.root);
    rig.music
        .update(Msg::Library(Found::Fresh(lib, Vec::new())));
    rig.wait(100);
    assert!(rig.music.book().is_some(), "the book is back");
    // A fresh library starts the screens over; SELECT goes back to what plays.
    rig.keys(&[Button::Select]);
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
    assert!(!rig.music.pocket.locked, "not on a list");
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
    assert!(rig.music.pocket.locked, "either order");
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
    // Once to lock, twice to unlock, each a buzz as long as the gap after it.
    let pattern = |rig: &mut Rig| {
        let mut seen = vec![rig.music.rumble()];
        for _ in 0..10 {
            rig.waited.set(rig.waited.get() + pocket::BUZZ / 2);
            seen.push(rig.music.rumble());
        }
        seen.into_iter().flatten().collect::<Vec<_>>()
    };
    rig.waited.set(rig.waited.get() + Duration::from_secs(5));
    assert_eq!(rig.music.rumble(), Some(0), "the motor stops");
    combo(&mut rig, Button::L2, Button::R2);
    assert!(!rig.music.pocket.locked, "either combo unlocks");
    assert_eq!(
        pattern(&mut rig),
        [u16::MAX, 0, u16::MAX, 0],
        "twice, to unlock"
    );
    // A lock lasts only as long as the screen is off.
    combo(&mut rig, Button::L1, Button::R1);
    rig.music.set_dark(true);
    rig.music.set_dark(false);
    assert!(!rig.music.pocket.locked, "the lid opened unlocks");
    // The screen going off forgets the keys held: one alone is no combo after.
    rig.down(Button::L1);
    rig.music.set_dark(true);
    rig.down(Button::R1);
    rig.wait(600);
    assert!(!rig.music.pocket.locked);
}

#[test]
fn under_the_lid_a_trigger_plays_or_pauses_a_shoulder_changes_song_and_locked_neither() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
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
    assert!(rig.music.pocket.locked);
    rig.told();
    rig.keys(&[Button::R2, Button::L1]);
    assert_eq!(rig.told(), [], "nothing changes in a pocket");
}

#[test]
fn the_pocket_combo_tapped_plays_or_pauses_and_neither_key_acts_alone() {
    let mut rig = Rig::new(&["Music/X/A/01 one.mp3", "Music/X/A/02 two.mp3"]);
    rig.keys(&[Button::Down, Button::Down, Button::A, Button::A, Button::A]);
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
        assert!(
            told == [Command::Pause] || told == [Command::Resume],
            "{first:?} with {second:?}, dark {dark}: {told:?}"
        );
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
    rig.keys(&[Button::Down, Button::Down, Button::A]);
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
fn up_on_now_playing_steps_the_repeat_round_and_the_corner_says_so() {
    let mut rig = Rig::new(&BOOK[2..]);
    rig.keys(&[Button::A]);
    let mut said = Vec::new();
    for key in [
        Button::Up,
        Button::Up,
        Button::Up,
        Button::Down,
        Button::Down,
    ] {
        rig.keys(&[key]);
        said.push(rig.screen().corner);
    }
    let shuffle = |on| text::notice(&Notice::Shuffle(on));
    let repeat = |r| text::notice(&Notice::Repeat(r));
    assert_eq!(
        said,
        [
            repeat(Repeat::All),
            repeat(Repeat::One),
            repeat(Repeat::Off),
            // Shuffle Library played it shuffled: the first Down puts it in order.
            shuffle(false),
            shuffle(true)
        ]
    );
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
    music.player.on(&music.library, end);
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
fn the_pocket_combo_as_the_device_logged_it_pauses_with_no_seek_or_skip() {
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
    assert_eq!(rig.told(), [Command::Pause], "paused, nothing else");
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
    assert!(rig.music.pocket.locked);
    rig.told();
    combo(&mut rig, 100);
    assert_eq!(rig.told(), [], "a squeeze in a pocket changes nothing");
    assert!(rig.music.pocket.locked);
    combo(&mut rig, 600);
    assert!(!rig.music.pocket.locked, "held, it unlocks");
}
