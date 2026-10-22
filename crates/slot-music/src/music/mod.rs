//! The music player as slot holds it: the state behind every screen, and the shell around it.
//! What happens comes in between frames, from the player, the library scan and the worker, and
//! from keys as they go down and up; the state acts on it and says what it needs done outside,
//! as effects, which the shell carries out once it has acted. The state never waits on the card
//! or a thread, so a frame is never held up.

mod act;
mod art;
mod books;
mod controller;
mod engine;
mod input;
mod loading;
mod lyrics;
#[cfg(feature = "perf")]
mod perf;
mod pocket;
mod read_ahead;
mod transport;
mod visualizer;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::books::chapters::Chapter;
use crate::browse::{Browser, Context, View};
use crate::library::TrackId;
use crate::library::{scan, Library};
use crate::lyrics::Lyrics;
use crate::playback::player::Player;
use crate::playback::recent::Recent;
pub(crate) use read_ahead::ReadAhead;
pub(crate) use visualizer::{BAR_STEPS, FRAME_S};

use crate::integrations::scrobbler::Scrobbler;
use crate::settings::Settings;
use crate::store::equalizer::Presets;
use crate::store::queue::SavedQueue;
use crate::store::settings::SETTINGS_FILE;
use crate::store::{Card, Kept, Store};
use crate::text::Notice;
use art::Pictures;
use books::Books;
pub use controller::{Button, Input};
use engine::{Engine, Start};
use input::Buttons;
use loading::{spawn_scan, Found, Scan};
use slot_player::{Command, Event, Handle, PlayState};
use transport::Transport;
use worker::{Done, Job, Worker};

/// What the state asks of the world outside it.
pub(crate) enum Effect {
    /// Tell the player.
    Player(Command),
    /// Start a fresh player, the last having gone.
    Restart,
    /// Write a file whole: its path from the card's root, or its own.
    Write(PathBuf, String),
    /// Set one key of `music.txt`, the rest of it as it was.
    Setting(&'static str, String),
    /// Add `line` onto the end of `file`, from the card's root, begun with `header` if new.
    Append {
        file: &'static str,
        header: &'static str,
        line: String,
    },
    /// Give the worker a job, under the library's generation it was asked in.
    Work(u32, Job),
    /// Start the performance log, or stop it.
    #[cfg(feature = "perf")]
    PerfLog(bool),
    /// Log what happens as well as what went wrong, or only that.
    Log(bool),
}

/// What is shown for a moment, and since when: a notice in the corner, and the letter a list
/// last jumped to, large.
#[derive(Default)]
pub(crate) struct Flash {
    pub(crate) notice: Option<(Notice, Instant)>,
    pub(crate) letter: Option<(char, Instant)>,
}

/// What is read ahead for what plays, off the frame: the lyrics of the song playing and the
/// next, and the song they were last shown for, to start a new one's from the top; the
/// chapters of the book's part playing and the next.
#[derive(Default)]
struct Ahead {
    lyrics: ReadAhead<Option<Lyrics>>,
    lyrics_for: Option<TrackId>,
    chapters: ReadAhead<Vec<Chapter>>,
}

/// What reaches the state from outside between frames.
enum Msg {
    Player(Event),
    Library(Found),
    Worker(u32, Done),
    /// A write to the card failed.
    SaveFailed,
}

/// The outside as the state sees it, read once as something comes in: the moment, for what is
/// timed; the seconds since 1970, for what is kept; and a number new each time, for a shuffle.
#[derive(Copy, Clone)]
pub(crate) struct Now {
    pub(crate) at: Instant,
    pub(crate) wall: u64,
    pub(crate) seed: u64,
}

/// The system's time.
pub(crate) fn system_now() -> Now {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Now {
        at: Instant::now(),
        wall: since.as_secs(),
        seed: (since.as_nanos() as u64).max(1),
    }
}

/// The world outside the state: the player's thread, the card and the worker.
struct Shell {
    engine: Box<dyn Engine>,
    /// Where the time comes from: the system, or a test's own.
    clock: Box<dyn Fn() -> Now>,
    /// Starts a player: the device's, or one a test gives.
    start: Start,
    store: Store,
    worker: Worker,
    /// The performance log, while it runs.
    #[cfg(feature = "perf")]
    perf: Option<perf::PerfLog>,
}

/// The music player: the library, the screens, the queue and the player.
pub struct Music {
    pub(crate) root: PathBuf,
    shell: Shell,
    /// What the state asked since the shell last carried it out.
    out: Vec<Effect>,
    pub(crate) library: Library,
    pub(crate) scan: Scan,
    pub(crate) pictures: Pictures,
    pub(crate) browser: Browser,
    pub(crate) player: Player,
    pub(crate) transport: Transport,
    pub(crate) books: Books,
    pub(crate) settings: Settings,
    /// The player is gone, its thread failed: the next thing played starts another.
    pub(crate) gone: bool,
    /// Bumped by a rescan; the worker's results from before are dropped.
    generation: u32,
    /// The time, as it was when something last came in.
    pub(crate) now: Now,
    /// What is shown for a moment: a notice in the corner, the letter a list jumped to.
    pub(crate) flash: Flash,
    /// The buttons, read into keys, and the pocket lock.
    pub(crate) buttons: Buttons,
    /// The lyrics and chapters read ahead for what plays.
    ahead: Ahead,
    /// The songs played last.
    pub(crate) recent: Recent,
    /// The equalizer's presets, from the card.
    pub(crate) equalizer: Presets,
    pub(crate) visualizer: visualizer::Visualizer,
    /// What the scrobbler log was told of the song playing.
    scrobbler: Scrobbler,
    /// The screen is off: only the shoulders act.
    dark: bool,
}

impl Music {
    /// `art_px` is the size album pictures are shrunk to, `thumb_px` a list row's thumbnails;
    /// `player` starts the player to play
    /// through, the device's (`Handle::slot`) or one on any output (`Handle::on`), the frontend's
    /// to choose: the music knows no device. Made, and polled, on the frame loop's thread, which
    /// the worker wakes as it has something to show.
    pub fn spawn(root: &Path, art_px: u32, thumb_px: u32, player: fn() -> Handle) -> Music {
        let start: Start = Box::new(move || Box::new(player()));
        Music::with(root, (art_px, thumb_px), start, Box::new(system_now))
    }

    /// As `spawn`, pictures shrunk to `px` (Now Playing's, a row's), playing through what
    /// `start` starts, its time from `clock`.
    pub(crate) fn with(
        root: &Path,
        (art_px, thumb_px): (u32, u32),
        start: Start,
        clock: Box<dyn Fn() -> Now>,
    ) -> Music {
        let now = clock();
        let engine = start();
        let store = Store::open(root);
        let mut settings = store.settings();
        crate::logging::start(settings.log);
        let (equalizer, eq) = store.equalizer();
        settings.eq = eq;
        let mut player = Player::default();
        player.set_repeat(settings.repeat);
        // Known before the library is read, so the main screen does not change its mind.
        if crate::store::queue::saved(root) {
            player.wait_for_saved();
        }
        // The first start opens on Help, once.
        let (mut browser, mut out) = (Browser::default(), Vec::new());
        if !settings.help_seen {
            browser.push(View::Help, 0);
            settings.help_seen = true;
            out.push(Effect::Setting("help", "seen".into()));
        }
        let progress = Arc::new(scan::Progress::default());
        Music {
            root: root.to_path_buf(),
            out,
            library: Library::default(),
            scan: Scan {
                rx: Some(spawn_scan(root, false, progress.clone(), thumb_px)),
                thumb_px,
                ready: false,
                queue_text: Some(store.read(SavedQueue::FILE)),
                recent_text: Some(store.read(Recent::FILE)),
                replaying: None,
                play_when_read: false,
                places: Vec::new(),
                queue_saved: None,
                progress,
            },
            books: Books {
                shelf: store.load(&Library::default()),
                aside: None,
                back: None,
            },
            shell: Shell {
                engine,
                start,
                clock,
                store,
                // Made on the frame loop, which rests by parking: the worker wakes it.
                worker: Worker::spawn(art_px, thumb_px, std::thread::current()),
                #[cfg(feature = "perf")]
                perf: settings.perf.then(|| perf::PerfLog::start(root)),
            },
            pictures: Pictures::default(),
            browser,
            player,
            transport: Transport::default(),
            settings,
            gone: false,
            generation: 0,
            now,
            flash: Flash::default(),
            buttons: Buttons::default(),
            ahead: Ahead::default(),
            recent: Recent::default(),
            equalizer,
            scrobbler: Scrobbler::default(),
            visualizer: Default::default(),
            dark: false,
        }
    }

    /// Once a frame: what came in, then what plays on of itself. Idle, with nothing playing,
    /// fading, timed or held, only the pictures go on.
    pub fn poll(&mut self, game: bool) {
        // The frame's own time, for the perf log; the state's, read once.
        #[cfg(feature = "perf")]
        let began = Instant::now();
        self.stamp();
        let (status, inbox) = (self.shell.engine.status(), self.inbox());
        self.frame(status, inbox, game);
        self.run();
        #[cfg(feature = "perf")]
        self.note_frame(began, status.work);
    }

    /// A frame of the state, given what came in: the player as it is now, and the messages.
    /// It asks the world only by `Effect`, which `poll` then carries out.
    fn frame(&mut self, status: slot_player::Status, inbox: Vec<Msg>, game: bool) {
        // Where the player is, read rather than told.
        self.player.observe(status, self.now.at);
        let quiet = inbox.is_empty() && self.idle();
        for msg in inbox {
            self.update(msg);
        }
        if !quiet {
            self.tick();
            self.count_played();
            // What to go on into with no gap, or fading, as the queue now stands.
            let next = self.player.upcoming(&self.library, true);
            self.send(next);
            self.note_book();
        }
        // With the screen off nothing is shown: no lyrics or pictures to read for it.
        if !self.dark {
            self.read_lyrics();
        }
        self.read_chapters();
        self.tell_sound();
        self.tell_visualizer(status.spectrum);
        self.keep_queue();
        if !self.dark {
            self.show_art(game);
        }
    }

    /// While the performance log runs: how long this frame took, and what was shown.
    #[cfg(feature = "perf")]
    fn note_frame(&self, began: Instant, work: slot_player::Work) {
        let Some(perf) = &self.shell.perf else {
            return;
        };
        perf.stats.poll.add(began.elapsed());
        *perf
            .stats
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = work;
        // The song playing by its file, to tell which a cost went with.
        let song = self
            .player
            .current()
            .and_then(|t| self.library.tracks[t].path.file_name());
        let what = format!(
            "{:?} {:?}{} {}",
            self.browser.view(),
            self.player.state(),
            if self.dark { " dark" } else { "" },
            song.map(|f| f.to_string_lossy()).unwrap_or_default()
        );
        *perf
            .stats
            .what
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = what;
    }

    #[cfg_attr(not(feature = "perf"), allow(unused_variables))]
    /// While the performance log runs: how long the frontend took to draw this frame's screen.
    pub fn note_draw(&self, took: std::time::Duration) {
        #[cfg(feature = "perf")]
        if let Some(perf) = &self.shell.perf {
            perf.stats.glue.add(took);
        }
    }

    /// While the performance log runs: how long the frontend took to tell whether a frame shows
    /// anything new, building what it would show to compare with the last.
    #[cfg_attr(not(feature = "perf"), allow(unused_variables))]
    pub fn note_compare(&self, took: std::time::Duration) {
        #[cfg(feature = "perf")]
        if let Some(perf) = &self.shell.perf {
            perf.stats.compare.add(took);
        }
    }

    /// While the performance log runs: how long building `screen` took.
    #[cfg_attr(not(feature = "perf"), allow(unused_variables))]
    pub(crate) fn note_screen(&self, took: std::time::Duration) {
        #[cfg(feature = "perf")]
        if let Some(perf) = &self.shell.perf {
            perf.stats.screen.add(took);
        }
    }

    /// What came in since the last frame.
    fn inbox(&mut self) -> Vec<Msg> {
        let mut inbox: Vec<Msg> = self
            .shell
            .engine
            .events()
            .into_iter()
            .map(Msg::Player)
            .collect();
        while let Some(rx) = &self.scan.rx {
            match rx.try_recv() {
                Ok(found) => inbox.push(Msg::Library(found)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.scan.rx = None;
                    // Gone before a library came: it failed, and is waited on no more.
                    if !self.scan.ready {
                        inbox.push(Msg::Library(Found::Failed));
                    }
                }
            }
        }
        if self.shell.store.failed() {
            inbox.push(Msg::SaveFailed);
        }
        let done = self.shell.worker.done();
        inbox.extend(done.map(|(generation, done)| Msg::Worker(generation, done)));
        inbox
    }

    /// Nothing to do of itself this frame.
    fn idle(&self) -> bool {
        self.player.state() != PlayState::Playing
            && self.transport.fade.is_none()
            && self.transport.sleep.left(self.now.at).is_none()
            && !self.buttons.keys.held()
            && !self.buttons.pocket.buzzing()
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Player(event) => self.on_player(event),
            Msg::Library(found) => self.on_found(found),
            Msg::Worker(generation, done) if generation == self.generation => match done {
                Done::Chapters(track, chapters) => self.chapters_read(track, chapters),
                Done::Lyrics(track, lyrics) => self.ahead.lyrics.put(track, lyrics),
                done => self.on_art(done),
            },
            Msg::Worker(..) => {}
            // Said each time, so a card full or locked is not mistaken for one that keeps.
            Msg::SaveFailed => self.tell(Notice::CantSave),
        }
    }

    fn on_player(&mut self, event: Event) {
        match event {
            // Gone, it is stopped; the next thing played starts another.
            Event::Gone => {
                self.gone = true;
                self.player.gone();
            }
            // The device stopped, not the file: paused where it was, and the corner says so.
            Event::Lost(why) => {
                log::warn!("{why}");
                self.tell(Notice::NoSound);
            }
            event => {
                // A song that cannot be played is skipped; the corner says why the music moved
                // on.
                if matches!(event, Event::Error(_)) {
                    self.tell(Notice::Unplayable);
                }
                let ends = self.ends_book(&event);
                let command = self.player.on_event(&self.library, event);
                self.send(command);
                if ends {
                    self.finish_book();
                }
            }
        }
    }

    /// Carries out what the state asked.
    fn run(&mut self) {
        let shell = &mut self.shell;
        for effect in self.out.drain(..) {
            match effect {
                Effect::Player(command) => {
                    log::info!("{command:?}");
                    #[cfg(feature = "perf")]
                    if let (
                        Some(perf),
                        Command::Play { .. } | Command::Cue { .. } | Command::Next(Some(_)),
                    ) = (&shell.perf, &command)
                    {
                        perf.stats
                            .opened
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    shell.engine.send(command);
                }
                Effect::Restart => shell.engine = (shell.start)(),
                Effect::Write(file, text) => shell.store.write(file, text),
                Effect::Setting(key, value) => shell.store.set(SETTINGS_FILE, key, value),
                Effect::Append { file, header, line } => shell.store.append(file, header, line),
                Effect::Work(generation, job) => shell.worker.send(generation, job),
                #[cfg(feature = "perf")]
                Effect::PerfLog(on) => {
                    shell.perf = on.then(|| perf::PerfLog::start(&self.root));
                }
                Effect::Log(detailed) => crate::logging::start(detailed),
            }
        }
    }

    /// Waits until everything kept is on the card: before powering off.
    pub fn flush(&mut self) {
        self.run();
        self.shell.store.flush();
    }

    pub fn owns_audio(&self) -> bool {
        self.player.owns_audio()
    }

    /// Whether the player may still be sounding.
    pub fn audible(&self) -> bool {
        self.player.audible()
    }

    /// How long until it changes of itself: at once, frame by frame, while the motor buzzes or
    /// the sound fades; on a screen that follows the song (Now Playing, a book's, the lyrics),
    /// when that next shows it moved on. None, until something comes in: the frontend may rest
    /// until then.
    pub fn due_in(&self) -> Option<Duration> {
        if self.buttons.pocket.buzzing() || self.transport.fade.is_some() {
            return Some(Duration::ZERO);
        }
        // A millisecond past the step: on it to the last bit, a time may still round short.
        let steps = (!self.dark)
            .then(|| crate::screens::steps_in(self))
            .flatten();
        let after = |s: f64| Duration::try_from_secs_f64(s.max(0.0)).unwrap_or(Duration::MAX);
        steps.map(|s| after(s).saturating_add(STEP_PAST))
    }

    /// The music screen opened from slot's menu: on Now Playing while something plays, when
    /// Open on Now Playing says so; else where it was left.
    pub fn open(&mut self) {
        let playing = self.player.state() == PlayState::Playing;
        if self.settings.open_playing && playing && self.browser.view() != View::NowPlaying {
            self.browser.home();
            self.browser.push(View::NowPlaying, 0);
        }
    }

    /// Whether music carries on with the screen off (`screen_off = play`).
    pub fn plays_screen_off(&self) -> bool {
        self.settings.screen_off.plays()
    }

    /// Shows `notice` in the corner for a moment.
    fn tell(&mut self, notice: Notice) {
        self.flash.notice = Some((notice, self.now.at));
    }

    /// Tells the player `command`. A player gone, its thread failed, is started again to play.
    fn send(&mut self, command: Option<Command>) {
        let Some(command) = command else {
            return;
        };
        let plays = matches!(
            command,
            Command::Play { .. } | Command::Cue { .. } | Command::Resume
        );
        if plays && self.gone {
            self.out.push(Effect::Restart);
            self.transport.sound = None;
            self.gone = false;
        }
        self.out.push(Effect::Player(command));
    }

    /// The card's root and the library on it, to write paths by.
    fn card(&self) -> Card<'_> {
        Card {
            root: &self.root,
            lib: &self.library,
        }
    }

    /// What the screens and their keys read.
    pub(crate) fn context(&self) -> Context<'_> {
        Context {
            lib: &self.library,
            queue: self.player.queue(),
            settings: self.settings,
            sleep: self.transport.sleep.left(self.now.at),
            shelf: &self.books.shelf,
            book: self.book().map(|(b, _)| b),
            back: self.books.back.map(|s| s.pos),
            lyrics: self.lyrics_at(),
            recent: &self.recent.tracks,
            shuffled: self.player.shuffled(),
            chapters: &self.ahead.chapters,
            eq: self.equalizer.name(self.settings.eq),
            thumbs: &self.pictures.thumbs,
            now: self.now.at,
        }
    }

    /// Reads the time, as something comes in: the only place the state learns it.
    pub(crate) fn stamp(&mut self) {
        self.now = (self.shell.clock)();
    }

    /// Songs the scan has gone through, of those it found.
    pub(crate) fn reading(&self) -> (usize, usize) {
        self.scan.progress.get()
    }
}

#[cfg(test)]
mod tests;

/// How far past a step of the song a frame is drawn for it: too little to see, enough that the
/// step has surely come.
const STEP_PAST: Duration = Duration::from_millis(1);
