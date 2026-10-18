//! The music player as slot holds it: the state behind every screen, and the shell around it.
//! What happens comes in between frames, from the player, the library scan and the worker, and
//! from keys as they go down and up; the state acts on it and says what it needs done outside,
//! as effects, which the shell carries out once it has acted. The state never waits on the card
//! or a thread, so a frame is never held up.

mod act;
mod art;
mod books;
mod input;
mod loading;
mod lyrics;
mod perf;
mod pocket;
mod read_ahead;
mod transport;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::Instant;

use crate::books::chapters::Chapter;
use crate::browse::keys::Keys;
use crate::browse::{Browser, Context, Key};
use crate::library::{scan, Library};
use crate::playback::player::Player;
use crate::playback::recent::Recent;
use read_ahead::ReadAhead;

use crate::settings::{Settings, Theme};
use crate::store::queue::SavedQueue;
use crate::store::settings::SETTINGS_FILE;
use crate::store::{Card, Kept, Store};
use crate::text::Notice;
use art::Pictures;
use books::Books;
use loading::{spawn_scan, Found, Scan};
use pocket::PocketLock;
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
    /// Give the worker a job, under the library's generation it was asked in.
    Work(u32, Job),
    /// Start the performance log, or stop it.
    PerfLog(bool),
}

/// What reaches the state from outside between frames.
enum Msg {
    Player(Event),
    Library(Found),
    Worker(u32, Done),
}

/// The world outside the state: the player's thread, the card and the worker.
struct Shell {
    engine: Handle,
    /// Starts a player: the device's, or one a test gives.
    start: fn() -> Handle,
    store: Store,
    worker: Worker,
    /// The performance log, while it runs.
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
    pub(crate) notice: Option<(Notice, Instant)>,
    /// The letter a list last jumped to, shown large for a moment.
    pub(crate) letter: Option<(char, Instant)>,
    /// The key down: tapped, held or repeating.
    pub(crate) keys: Keys<Key>,
    pub(crate) pocket: PocketLock,
    pub(crate) lyrics: lyrics::SongLyrics,
    /// The chapters of the book's part playing and the next, read ahead.
    chapters: ReadAhead<Vec<Chapter>>,
    /// The songs played last.
    pub(crate) recent: Recent,
    /// Why the sound last failed, for the Debug page.
    last_error: Option<String>,
    /// The screen is off: only the shoulders act.
    dark: bool,
}

impl Music {
    /// `art_px` is the size album pictures are shrunk to.
    pub fn spawn(root: &Path, art_px: u32) -> Music {
        Music::with_player(root, art_px, Handle::slot)
    }

    /// As `spawn`, playing through what `start` starts.
    pub(crate) fn with_player(root: &Path, art_px: u32, start: fn() -> Handle) -> Music {
        let engine = start();
        let store = Store::open(root);
        let settings = store.settings();
        let mut player = Player::default();
        player.set_repeat(settings.repeat);
        // Known before the library is read, so the main screen does not change its mind.
        if crate::store::queue::saved(root) {
            player.wait_for_saved();
        }
        engine.send(Command::Mono(settings.mono));
        let progress = Arc::new(scan::Progress::default());
        Music {
            root: root.to_path_buf(),
            out: Vec::new(),
            library: Library::default(),
            scan: Scan {
                rx: Some(spawn_scan(root, false, progress.clone())),
                ready: false,
                queue_text: Some(store.read(SavedQueue::FILE)),
                recent_text: Some(store.read(Recent::FILE)),
                replaying: None,
                play_when_read: false,
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
                store,
                worker: Worker::spawn(art_px),
                perf: settings.perf.then(|| perf::PerfLog::start(root)),
            },
            pictures: Pictures::default(),
            browser: Browser::default(),
            player,
            transport: Transport::default(),
            settings,
            gone: false,
            generation: 0,
            notice: None,
            letter: None,
            keys: Keys::default(),
            pocket: PocketLock::default(),
            lyrics: Default::default(),
            chapters: ReadAhead::new(2),
            recent: Recent::default(),
            last_error: None,
            dark: false,
        }
    }

    /// Once a frame: what came in, then what plays on of itself. Idle, with nothing playing,
    /// fading, timed or held, only the pictures go on.
    pub fn poll(&mut self, game: bool) {
        let began = Instant::now();
        // Where the player is, read rather than told.
        self.player.observe(self.shell.engine.status());
        let inbox = self.inbox();
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
        self.keep_queue();
        if !self.dark {
            self.show_art(game);
        }
        self.run();
        self.note_frame(began);
    }

    /// While the performance log runs: how long this frame took, and what was shown.
    fn note_frame(&self, began: Instant) {
        let Some(perf) = &self.shell.perf else {
            return;
        };
        perf.stats.poll.add(began.elapsed());
        let what = format!(
            "{:?} {:?}{}",
            self.browser.view(),
            self.player.state(),
            if self.dark { " dark" } else { "" }
        );
        *perf
            .stats
            .what
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = what;
    }

    /// While the performance log runs: how long the frontend took to draw this frame's screen.
    pub fn note_draw(&self, took: std::time::Duration) {
        if let Some(perf) = &self.shell.perf {
            perf.stats.glue.add(took);
        }
    }

    /// While the performance log runs: how long building `screen` took.
    pub(crate) fn note_screen(&self, took: std::time::Duration) {
        if let Some(perf) = &self.shell.perf {
            perf.stats.screen.add(took);
        }
    }

    /// What came in since the last frame.
    fn inbox(&mut self) -> Vec<Msg> {
        let mut inbox: Vec<Msg> = self.shell.engine.events().map(Msg::Player).collect();
        while let Some(rx) = &self.scan.rx {
            match rx.try_recv() {
                Ok(found) => inbox.push(Msg::Library(found)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => self.scan.rx = None,
            }
        }
        let done = self.shell.worker.done();
        inbox.extend(done.map(|(generation, done)| Msg::Worker(generation, done)));
        inbox
    }

    /// Nothing to do of itself this frame.
    fn idle(&self) -> bool {
        self.player.state() != PlayState::Playing
            && self.transport.fade.is_none()
            && self.transport.sleep.left().is_none()
            && !self.keys.held()
            && !self.pocket.buzzing()
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Player(event) => self.on_player(event),
            Msg::Library(found) => self.on_found(found),
            Msg::Worker(generation, done) if generation == self.generation => match done {
                Done::Chapters(track, chapters) => {
                    self.chapters.put(track, chapters);
                    self.read_chapters();
                }
                done => self.on_art(done),
            },
            Msg::Worker(..) => {}
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
                self.last_error = Some(why);
                self.tell(Notice::NoSound);
            }
            event => {
                // A song that cannot be played is skipped; the corner says why the music moved
                // on.
                if let Event::Error(why) = &event {
                    self.last_error = Some(why.clone());
                    self.tell(Notice::Unplayable);
                }
                let ends = self.ends_book(&event);
                let command = self.player.on(&self.library, event);
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
                    eprintln!("slot: music: {command:?}");
                    shell.engine.send(command);
                }
                Effect::Restart => shell.engine = (shell.start)(),
                Effect::Write(file, text) => shell.store.write(file, text),
                Effect::Setting(key, value) => shell.store.set(SETTINGS_FILE, key, value),
                Effect::Work(generation, job) => shell.worker.send(generation, job),
                Effect::PerfLog(on) => {
                    shell.perf = on.then(|| perf::PerfLog::start(&self.root));
                }
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

    /// Whether music carries on with the screen off (`screen_off = play`).
    pub fn plays_screen_off(&self) -> bool {
        self.settings.play_screen_off
    }

    /// The Theme setting: slot's appearance, or always dark or light.
    pub fn theme(&self) -> Theme {
        self.settings.theme
    }

    /// Shows `notice` in the corner for a moment. Nothing for the player.
    fn tell(&mut self, notice: Notice) -> Option<Command> {
        self.notice = Some((notice, Instant::now()));
        None
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
            self.out
                .push(Effect::Player(Command::Mono(self.settings.mono)));
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
            sleep: self.transport.sleep.left(),
            shelf: &self.books.shelf,
            book: self.book().map(|(b, _)| b),
            back: self.books.back.map(|s| s.pos),
            lyrics: self.lyrics_at(),
            recent: &self.recent.tracks,
            last_error: self.last_error.as_deref(),
            shuffled: self.player.shuffled(),
        }
    }

    /// Songs the scan has gone through, of those it found.
    pub(crate) fn reading(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::Relaxed;
        (
            self.scan.progress.done.load(Relaxed),
            self.scan.progress.of.load(Relaxed),
        )
    }
}

pub(crate) fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::AlbumId;
    use crate::library::BookId;
    use crate::playback::fade::Fade;
    use crate::playback::fade::Fading;
    use crate::playback::queue::Queue;
    use slot_player::Status;
    use std::time::Duration;

    /// Each buzz, as the settings have it by default.
    const BUZZ: Duration = Duration::from_millis(250);

    /// An output that plays nothing, so no test touches a sound device.
    struct Silent;

    impl slot_player::Output for Silent {
        fn write(&mut self, _: &[f32]) -> Result<(), String> {
            Ok(())
        }

        fn delay(&mut self) -> u64 {
            0
        }

        fn flush(&mut self) {}

        fn release(&mut self) {}
    }

    fn music(root: &Path) -> Music {
        Music::with_player(root, 64, || Handle::on(Silent))
    }

    #[test]
    fn a_book_the_sleep_timer_stops_goes_back_two_minutes() {
        let d = tempfile::tempdir().unwrap();
        let book = d.path().join("Audiobooks/Kipling - Just So/01 whale.mp3");
        std::fs::create_dir_all(book.parent().unwrap()).unwrap();
        std::fs::write(&book, b"").unwrap();
        let mut music = music(d.path());
        music.library = Library::scan(d.path());
        let parts = music.library.albums[music.library.books[0].album]
            .tracks
            .clone();
        music
            .player
            .play(&music.library, Queue::new(parts, 0, None));
        music.player.observe(Status {
            state: PlayState::Playing,
            at: 300.0,
            len: 600.0,
        });
        music.transport.fade = Some(Fade::new(1.0, Fading::Asleep, Duration::ZERO));
        music.tick();
        assert_eq!(music.player.time().0, 180.0);
        assert!(music.transport.fade.is_none(), "once");
    }

    #[test]
    fn a_book_playing_through_update_library_plays_on_as_music_does() {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "Audiobooks/Kipling - Just So/01 whale.mp3",
            "Music/X/A/01 one.mp3",
        ] {
            let path = d.path().join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        let mut music = music(d.path());
        music.on_found(Found::Fresh(Library::scan(d.path()), Vec::new()));
        let play = music.play_book(BookId(0), false);
        music.send(play);
        music.player.observe(Status {
            state: PlayState::Playing,
            at: 1.0,
            len: 60.0,
        });
        music.run();
        music.rescan(false);
        music.on_found(Found::Fresh(Library::scan(d.path()), Vec::new()));
        assert!(music.book().is_some(), "the book is back");
        assert_eq!(music.player.state(), PlayState::Playing, "and plays on");
        let stops = music
            .out
            .iter()
            .any(|e| matches!(e, Effect::Player(Command::Stop)));
        assert!(!stops, "the player is not stopped");
    }

    #[test]
    fn each_part_of_a_book_has_its_chapters_asked_for_once_however_it_is_reached() {
        let d = tempfile::tempdir().unwrap();
        for part in ["01 one.m4b", "02 two.m4b"] {
            let path = d.path().join("Audiobooks/Wilde - Tales").join(part);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        let mut music = music(d.path());
        music.on_found(Found::Fresh(Library::scan(d.path()), Vec::new()));
        let play = music.play_book(BookId(0), false);
        music.send(play);
        let asked = |music: &Music| {
            let jobs = music.out.iter().filter_map(|e| match e {
                Effect::Work(_, Job::Chapters { track, .. }) => Some(*track),
                _ => None,
            });
            jobs.collect::<Vec<_>>()
        };
        music.read_chapters();
        music.read_chapters();
        let parts = music.library.albums[music.library.books[0].album]
            .tracks
            .clone();
        assert_eq!(
            asked(&music),
            parts,
            "this part's and the next's, read ahead, once"
        );
        // On into the next part as the first runs out, as a gapless book goes.
        music.player.on(
            &music.library,
            Event::End(music.library.tracks[parts[0]].path.clone()),
        );
        music.read_chapters();
        assert_eq!(asked(&music), parts, "the next part's already asked");
        let chapters = vec![Chapter {
            start: 0.0,
            title: "One".into(),
        }];
        music.player.set_chapters(parts[1], chapters);
        assert_eq!(music.player.chapters(), 1, "there as the part plays");
    }

    #[test]
    fn a_song_s_lyrics_are_read_as_it_shows_once_and_the_hint_follows() {
        let d = tempfile::tempdir().unwrap();
        for song in ["01 one.mp3", "02 two.mp3"] {
            let path = d.path().join("Music/X/A").join(song);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        let lrc = d.path().join("Music/X/A/01 one.lrc");
        std::fs::write(&lrc, "[00:01.00]Hello").unwrap();
        let mut music = music(d.path());
        music.on_found(Found::Fresh(Library::scan(d.path()), Vec::new()));
        let tracks = music.library.albums[AlbumId(0)].tracks.clone();
        let play = music
            .player
            .play(&music.library, Queue::new(tracks, 0, None));
        music.send(play);
        music.browser.push(crate::browse::View::NowPlaying, 0);
        music.read_lyrics();
        assert!(
            music.lyrics_now().is_some(),
            "read at once, beside the song"
        );
        std::fs::remove_file(&lrc).unwrap();
        music.read_lyrics();
        assert!(music.lyrics_now().is_some(), "once a song: not read again");
        music.player.skip(&music.library, 1);
        music.read_lyrics();
        assert!(music.lyrics_now().is_none(), "the next has none");
    }

    #[test]
    fn a_player_started_again_is_one_the_shell_was_given() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static STARTS: AtomicUsize = AtomicUsize::new(0);
        fn counted() -> Handle {
            STARTS.fetch_add(1, Ordering::Relaxed);
            Handle::on(Silent)
        }
        let d = tempfile::tempdir().unwrap();
        let mut music = Music::with_player(d.path(), 64, counted);
        music.gone = true;
        music.send(Some(Command::Resume));
        music.run();
        assert_eq!(
            STARTS.load(Ordering::Relaxed),
            2,
            "the first, then one more like it"
        );
    }

    #[test]
    fn with_the_screen_off_no_lyrics_or_pictures_are_read() {
        let d = tempfile::tempdir().unwrap();
        let song = d.path().join("Music/X/A/01 one.mp3");
        std::fs::create_dir_all(song.parent().unwrap()).unwrap();
        std::fs::write(&song, b"").unwrap();
        let mut music = music(d.path());
        music.on_found(Found::Fresh(Library::scan(d.path()), vec![AlbumId(0)]));
        let tracks = music.library.albums[AlbumId(0)].tracks.clone();
        let play = music
            .player
            .play(&music.library, Queue::new(tracks, 0, None));
        music.send(play);
        music.browser.push(crate::browse::View::NowPlaying, 0);
        music.set_dark(true);
        music.out.clear();
        music.poll(false);
        let work = music.out.iter().any(|e| matches!(e, Effect::Work(..)));
        assert!(!work, "nothing read for a screen that is off");
    }

    #[test]
    fn the_performance_log_runs_only_while_it_is_on() {
        let d = tempfile::tempdir().unwrap();
        let mut music = music(d.path());
        assert!(music.shell.perf.is_none(), "off by default: nothing kept");
        music.out.push(Effect::PerfLog(true));
        music.run();
        assert!(music.shell.perf.is_some());
        music.poll(false);
        let kept = &music.shell.perf.as_ref().unwrap().stats;
        assert!(!kept.what.lock().unwrap().is_empty(), "the frame noted");
        music.out.push(Effect::PerfLog(false));
        music.run();
        assert!(music.shell.perf.is_none());
    }

    #[test]
    fn a_player_gone_is_started_again_to_play() {
        let d = tempfile::tempdir().unwrap();
        let mut music = music(d.path());
        music.send(Some(Command::Resume));
        let restarts = |m: &Music| m.out.iter().any(|e| matches!(e, Effect::Restart));
        assert!(
            !restarts(&music),
            "up from the start, before it first answers"
        );
        music.out.clear();
        music.gone = true;
        music.send(Some(Command::Pause));
        assert!(!music.out.iter().any(|e| matches!(e, Effect::Restart)));
        music.send(Some(Command::Resume));
        assert!(music.out.iter().any(|e| matches!(e, Effect::Restart)));
        assert!(!music.gone, "started again");
    }

    #[test]
    fn what_the_state_asks_waits_for_the_shell_and_nothing_is_asked_twice() {
        let d = tempfile::tempdir().unwrap();
        let mut music = music(d.path());
        music.tell_sound();
        let told = music.out.len();
        assert!(told > 0, "the sound, once");
        music.tell_sound();
        assert_eq!(music.out.len(), told, "not again while it stands");
        music.run();
        assert!(music.out.is_empty(), "carried out");
    }

    /// A held pocket combo, waited out as the frame loop would.
    fn hold_combo(music: &mut Music, first: Key, second: Key) {
        music.press(first);
        music.press(second);
        std::thread::sleep(Duration::from_millis(600));
        music.poll(false);
        music.release(Some(second));
        music.release(Some(first));
    }

    #[test]
    fn the_pocket_combo_held_locks_the_keys_with_a_buzz_and_held_again_unlocks_them() {
        let d = tempfile::tempdir().unwrap();
        let mut music = music(d.path());
        hold_combo(&mut music, Key::R1, Key::L1);
        assert!(music.pocket.locked, "either order");
        assert_eq!(music.rumble(), Some(u16::MAX), "a buzz says so");
        assert_eq!(music.rumble(), None, "told once");
        assert_eq!(
            music.screen().corner,
            "Locked",
            "and the corner, while it lasts"
        );
        assert!(!music.press(Key::B), "locked, the keys do nothing");
        music.release(Some(Key::B));
        std::thread::sleep(BUZZ);
        assert_eq!(music.rumble(), Some(0), "then the motor stops");
        std::thread::sleep(BUZZ);
        assert_eq!(music.rumble(), None, "once, to lock");
        hold_combo(&mut music, Key::L2, Key::R2);
        assert!(!music.pocket.locked, "either combo unlocks");
        // Twice, to unlock.
        let mut pattern = Vec::new();
        for _ in 0..16 {
            pattern.extend(music.rumble());
            std::thread::sleep(BUZZ / 3);
        }
        assert_eq!(pattern, [u16::MAX, 0, u16::MAX, 0], "twice, to unlock");
        // A lock lasts only as long as the screen is off.
        hold_combo(&mut music, Key::L1, Key::R1);
        music.set_dark(true);
        music.set_dark(false);
        assert!(!music.pocket.locked, "the lid opened unlocks");
        // The screen going off forgets the keys held: one alone is no combo after.
        music.press(Key::L1);
        music.set_dark(true);
        music.press(Key::R1);
        std::thread::sleep(Duration::from_millis(600));
        music.poll(false);
        assert!(!music.pocket.locked);
    }
}
