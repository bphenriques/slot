pub mod books;
mod browse;
mod covers;
mod fade;
mod keys;
mod library;
mod muse;
mod player;
mod playlists;
mod queue;
pub mod queue_file;
mod rows;
mod scan;
mod screens;
mod settings;
mod tags;
pub mod text;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use books::SHELF_FILE;
pub use browse::{
    per_page, Browser, Context, Key, Outcome, Page, QueueView, Shuffle, View, MENU_ROWS,
};
pub use keys::{Mode, Press};
pub use library::{
    track_number, track_title, Album, Artist, Book, Library, Record, Track, BOOKS_DIR, MUSIC_DIR,
};
pub use muse::{parse, Command, Event, Muse, MuseState, SOCKET};
pub use player::{Player, Repeat};
pub use playlists::{write as write_playlist, Playlist};
pub use queue::Queue;
pub use rows::{rows, Info, Row, RowIcon, RowText};
pub use scan::{cached as cached_library, scan as scan_library, Progress, CACHE_FILE};
pub use screens::{Body, Legend, ListBody, MainHold, NowPlayingBody, Screen, Status, Target};
pub use settings::{cycle, ListSize, Setting, Settings, Theme, SETTINGS_FILE};
pub use tags::Tags;

use books::Shelf;
use covers::{Covers, Done, Job, Worker};
use fade::{Fade, SleepTimer, FADE, SLEEP_FADE};
use keys::Keys;
use queue_file::{Writer, QUEUE_FILE};
use text::Notice;

/// The playing album's picture, scaled to fit the square the screen asked for.
pub struct Art {
    pub album: usize,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The music player: the library, the screens, the queue and Muse.
pub struct Music {
    root: PathBuf,
    library: Library,
    loading: Option<Receiver<Found>>,
    /// A library is shown: the cache's, or the card's.
    ready: bool,
    /// The queue to put back once the library is read, when it is not the saved one.
    queue_text: Option<String>,
    /// How far the scan running has got.
    reading: Arc<scan::Progress>,
    covers: Covers,
    browser: Browser,
    player: Player,
    settings: Settings,
    muse: Muse,
    queue_file: Writer,
    connected: bool,
    shown: Option<usize>,
    art: Option<Art>,
    worker: Worker,
    /// Bumped by a rescan; older picture results are dropped.
    generation: u32,
    /// The track playing when a rescan began.
    replaying: Option<PathBuf>,
    /// Play was pressed while the saved queue was still being read back.
    play_when_read: bool,
    notice: Option<(Notice, Instant)>,
    /// The letter a list last jumped to, shown large for a moment.
    letter: Option<(char, Instant)>,
    fade: Option<Fade>,
    sleep: SleepTimer,
    /// The key down: tapped, held or repeating.
    keys: Keys<Key>,
    /// Where each audiobook was left, and the music's queue set aside while one plays.
    shelf: Shelf,
    shelf_file: Writer,
    aside: Option<(Queue, f64)>,
    /// Where the book was before a skip, a chapter or a bookmark moved it: its book, part and
    /// seconds.
    back: Option<(usize, usize, f64)>,
    /// The screen is off: only the shoulders act.
    dark: bool,
}

impl Music {
    /// `art_px` is the size album pictures are shrunk to.
    pub fn spawn(root: &Path, art_px: u32) -> Music {
        let settings = Settings::read(root);
        let mut player = Player::default();
        player.set_repeat(settings.repeat);
        // Known before the library is read, so the main screen does not change its mind.
        if queue_file::saved(root) {
            player.wait_for_saved();
        }
        let reading = Arc::new(scan::Progress::default());
        Music {
            root: root.to_path_buf(),
            library: Library::default(),
            loading: Some(spawn_scan(root, false, reading.clone())),
            reading,
            ready: false,
            queue_text: None,
            covers: Covers::new(Vec::new()),
            browser: Browser::default(),
            player,
            settings,
            queue_file: Writer::spawn(root.join(QUEUE_FILE)),
            muse: Muse::spawn(PathBuf::from(SOCKET)),
            connected: false,
            shown: None,
            art: None,
            worker: Worker::spawn(art_px),
            generation: 0,
            replaying: None,
            play_when_read: false,
            notice: None,
            letter: None,
            fade: None,
            sleep: SleepTimer::default(),
            keys: Keys::default(),
            shelf: Shelf::read(root),
            shelf_file: Writer::spawn(root.join(books::SHELF_FILE)),
            aside: None,
            back: None,
            dark: false,
        }
    }

    /// Reads the library again: what changed, or every song when `full`. The queue is restored
    /// by path, and the playing track plays on if it is still there.
    fn rescan(&mut self, full: bool) {
        self.note_book();
        self.save_queue();
        self.replaying = match self.player.state() {
            MuseState::Playing => self
                .player
                .current()
                .map(|t| self.library.tracks[t].path.clone()),
            _ => None,
        };
        self.queue_text = Some(self.queue_text());
        self.player.forget();
        self.aside = None;
        self.unshow();
        self.library = Library::default();
        self.ready = false;
        self.reading = Arc::new(scan::Progress::default());
        self.loading = Some(spawn_scan(&self.root, full, self.reading.clone()));
    }

    /// The screens and pictures, whose numbers a new library no longer means.
    fn unshow(&mut self) {
        self.browser = Browser::default();
        self.covers = Covers::new(Vec::new());
        self.shown = None;
        self.art = None;
        self.generation += 1;
    }

    /// The music's queue as the queue file has it, by path.
    fn queue_text(&self) -> String {
        let (queue, at, pos) = self.music_snapshot();
        queue_file::encode(&self.root, &self.library, queue, at, pos)
    }

    /// Once a frame. Album pictures are made only when no `game` runs.
    pub fn poll(&mut self, game: bool) {
        while let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(Found::Cached(lib)) => self.install(lib, Vec::new()),
                // The cache was right: the library shown stands, and its pictures can be made.
                Ok(Found::Same(missing)) => {
                    self.covers = Covers::new(missing);
                    self.loading = None;
                }
                Ok(Found::Fresh(lib, missing)) => {
                    self.loading = None;
                    self.install(lib, missing);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }
        let events: Vec<Event> = self.muse.events().collect();
        for event in events {
            match event {
                Event::Connected(up) => {
                    self.connected = up;
                    if !up {
                        self.player
                            .on(&self.library, Event::State(MuseState::Stopped));
                    }
                }
                event => {
                    // A song Muse cannot play is skipped; the corner says why the music moved on.
                    if matches!(event, Event::Error(_)) {
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
        if !game {
            if let Some(job) = self.covers.next(&self.root, &self.library) {
                self.worker.send(self.generation, job);
            }
        }
        self.tick();
        self.keep_queue();
        self.note_book();
        self.show_art();
    }

    /// Acts on held keys, runs the sleep timer and ends fades.
    fn tick(&mut self) {
        if let Some(press) = self.keys.due() {
            self.act(press);
        }
        let chapter_ends = self
            .chapter_left()
            .is_some_and(|left| self.sleep.chapter_due(left));
        if self.sleep.due() || chapter_ends {
            self.fade_out(SLEEP_FADE);
        }
        if let Some(fade) = self.fade.filter(Fade::done) {
            self.fade = None;
            if fade.is_out() {
                self.pause();
            }
        }
    }

    /// Pauses through a fade while music plays.
    fn fade_out(&mut self, over: Duration) {
        match self.player.owns_audio() {
            true => self.fade = Some(Fade::new(self.volume_scale(), 0.0, over)),
            false => self.pause(),
        }
    }

    /// Pauses at once, or plays, fading in. A press during the sleep timer's fade turns it
    /// back.
    fn toggle(&mut self) -> Option<Command> {
        // A saved queue still being read back plays once it is.
        if self.player.queue().waiting {
            self.play_when_read ^= true;
            return None;
        }
        if self.fade.is_some_and(|f| f.is_out()) {
            self.fade = Some(Fade::new(self.volume_scale(), 1.0, FADE));
            return None;
        }
        // A book picks up a little back.
        let rewind = self.resume_book();
        self.send(rewind);
        let command = self.player.toggle(&self.library);
        if self.player.owns_audio() {
            self.fade = Some(Fade::new(0.0, 1.0, FADE));
        }
        command
    }

    /// What slot's volume is scaled by: the fade's level, 0 while a pause or stop is on its
    /// way to Muse, 1 otherwise.
    pub fn volume_scale(&self) -> f32 {
        match self.player.owns_audio() {
            true => self.fade.map_or(1.0, |f| f.level()),
            false => 0.0,
        }
    }

    /// `key` went down; true when it leaves the music screen. Some keys wait to see whether
    /// they are held, and some repeat.
    pub fn press(&mut self, key: Key) -> bool {
        let mode = match self.dark {
            true => screens::dark::mode(key),
            false => self.browser.mode(&self.context(), key),
        };
        self.keys
            .press(key, mode)
            .is_some_and(|press| self.act(press))
    }

    /// The screen went off or on: off, only the shoulders act, as a closed lid leaves them.
    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
        self.keys.cancel();
    }

    /// `key` went up, or with `None`, every key, as the music screen is left; true when a tap
    /// leaves the music screen.
    pub fn release(&mut self, key: Option<Key>) -> bool {
        let Some(key) = key else {
            self.keys.cancel();
            return false;
        };
        self.keys.release(key).is_some_and(|press| self.act(press))
    }

    /// A key tapped or held: what it does on the screen shown. True when it leaves the music
    /// screen.
    fn act(&mut self, press: Press<Key>) -> bool {
        // Field by field, so the browser can move while the rest is read.
        let cx = Context {
            lib: &self.library,
            queue: self.player.queue(),
            settings: self.settings,
            sleep: self.sleep.left(),
            shelf: &self.shelf,
            book: self.book().map(|(b, _)| b),
            back: self.back.map(|(_, _, pos)| pos),
        };
        let outcome = match (self.dark, press) {
            (true, press) => {
                let jump = cx.book.map(|_| cx.settings.jump as i32);
                screens::dark::act(press, jump)
            }
            (false, Press::Tap(key)) => self.browser.press(&cx, key),
            (false, Press::Held(key)) => self.browser.held(&cx, key),
        };
        if matches!(
            outcome,
            Outcome::Skip(_) | Outcome::Chapter(_) | Outcome::GoBookmark { .. }
        ) {
            self.remember_place();
        }
        let lib = &self.library;
        let command = match outcome {
            Outcome::None => None,
            Outcome::Leave => return true,
            Outcome::Rescan(full) => {
                self.rescan(full);
                return false;
            }
            Outcome::Toggle => {
                // Away from Now Playing, the corner says what START held did.
                let was = self.player.queue().playing;
                if self.browser.view() != View::NowPlaying && self.player.current().is_some() {
                    self.tell(if was { Notice::Paused } else { Notice::Playing });
                }
                self.toggle()
            }
            Outcome::Tell(notice) => self.tell(notice),
            Outcome::PlayBook { book, over } => self.play_book(book, over),
            Outcome::Finish(book) => {
                self.toggle_finished(book);
                None
            }
            Outcome::Speed(by) => self.step_speed(by),
            Outcome::Chapter(i) => self.go_chapter(i),
            Outcome::Bookmark => self.add_bookmark(),
            Outcome::GoBookmark { book, mark } => self.go_bookmark(book, mark),
            Outcome::RemoveBookmark { book, mark } => {
                self.remove_bookmark(book, mark);
                None
            }
            Outcome::Back => self.go_back(),
            // The queue stays, to resume; the screen-off timer can power off.
            Outcome::Stop => {
                self.fade_out(FADE);
                None
            }
            Outcome::Clear => {
                self.leave_book();
                self.fade = None;
                self.player.stop()
            }
            // The heart on Now Playing says it.
            Outcome::Favorite(t) => {
                self.library.toggle_favorite(t);
                None
            }
            Outcome::Remove(i) => {
                self.player.edit().remove(i);
                None
            }
            Outcome::Move(from, to) => {
                self.player.edit().move_to(from, to);
                None
            }
            Outcome::Letter(letter) => {
                self.letter = Some((letter, Instant::now()));
                None
            }
            Outcome::ShuffleQueue => {
                self.player.edit().toggle_shuffle(seed());
                None
            }
            Outcome::Change(Setting::SleepTimer, by) => {
                self.sleep.step(by, self.book().is_some());
                None
            }
            Outcome::Change(setting, by) => {
                self.settings.change(setting, by);
                self.player.set_repeat(self.settings.repeat);
                self.settings.write(&self.root);
                None
            }
            Outcome::Seek(by) => self.player.seek(by),
            Outcome::Skip(by) => self.player.skip(lib, by),
            Outcome::Jump(to) => self.player.jump(lib, to),
            Outcome::PlayNext(tracks) => {
                self.music_queue().play_next(&tracks);
                self.tell(Notice::Next(tracks.len()))
            }
            Outcome::Enqueue(tracks) => {
                self.music_queue().enqueue(&tracks);
                self.tell(Notice::Added(tracks.len()))
            }
            Outcome::Play {
                tracks,
                start,
                shuffle,
            } => {
                let queue = match shuffle {
                    Shuffle::Off => Queue::new(tracks, start, None),
                    Shuffle::Tracks => Queue::new(tracks, start, Some(seed())),
                    Shuffle::Groups(sizes) => Queue::groups(tracks, &sizes, seed()),
                };
                self.leave_book();
                self.player.play(&self.library, queue)
            }
        };
        self.send(command);
        self.keep_queue();
        self.note_book();
        false
    }

    /// Shows `notice` in the corner for a moment. Nothing for Muse.
    fn tell(&mut self, notice: Notice) -> Option<Command> {
        self.notice = Some((notice, Instant::now()));
        None
    }

    pub fn owns_audio(&self) -> bool {
        self.player.owns_audio()
    }

    /// Whether music carries on with the screen off (`screen_off = play`).
    pub fn plays_screen_off(&self) -> bool {
        self.settings.play_screen_off
    }

    /// The Theme setting: slot's appearance, or always dark or light.
    pub fn theme(&self) -> Theme {
        self.settings.theme
    }

    /// Whether Muse may still be sounding.
    pub fn audible(&self) -> bool {
        self.player.audible()
    }

    /// For a game starting or the screen going off: at once, without a fade.
    pub fn pause(&mut self) {
        self.fade = None;
        let command = self.player.pause();
        self.send(command);
        self.keep_queue();
    }

    /// Loads the playing album's picture when the album changes or its picture is filed.
    fn show_art(&mut self) {
        let album = self.player.current().map(|t| self.library.tracks[t].album);
        if album != self.shown {
            self.shown = album;
            // The last picture stays up while the next loads, to fade between them.
            if !album.is_some_and(|a| self.load_art(a)) {
                self.art = None;
            }
        }
        let done: Vec<(u32, Done)> = self.worker.done().collect();
        for (generation, done) in done {
            match done {
                _ if generation != self.generation => {}
                Done::Made(album) => {
                    self.covers.made();
                    if self.shown == Some(album)
                        && self.art.as_ref().map(|a| a.album) != Some(album)
                    {
                        self.load_art(album);
                    }
                }
                Done::Loaded(art) if self.shown == Some(art.album) => self.art = Some(art),
                Done::Loaded(_) => {}
            }
        }
    }

    /// Starts loading `album`'s picture; false when it has none yet.
    fn load_art(&self, album: usize) -> bool {
        let label = covers::label(&self.root, &self.library, album);
        let has = label.is_file();
        if has {
            self.worker
                .send(self.generation, Job::Load { album, label });
        }
        has
    }

    fn send(&mut self, command: Option<Command>) {
        if let Some(command) = command {
            eprintln!("slot: music: {}", command.to_string().replace('\t', " "));
            self.muse.send(command);
        }
    }

    /// Saves the queue when the player reports a change.
    fn keep_queue(&mut self) {
        if self.player.take_changed() {
            self.save_queue();
        }
    }

    /// Shows `lib`: the first one read, or a newer one from the card in place of the cache's,
    /// which carries the queue over by path and plays on if its song is still there.
    fn install(&mut self, lib: Library, missing: Vec<usize>) {
        if self.ready {
            self.queue_text = Some(self.queue_text());
            self.replaying = match self.player.state() {
                MuseState::Playing => self
                    .player
                    .current()
                    .map(|t| self.library.tracks[t].path.clone()),
                _ => None,
            };
            self.unshow();
        }
        eprintln!(
            "slot: music: {} artists, {} albums, {} tracks, {} to optimize",
            lib.artists.len(),
            lib.albums.len(),
            lib.tracks.len(),
            missing.len()
        );
        self.library = lib;
        self.covers = Covers::new(missing);
        self.ready = true;
        self.restore_queue();
        if std::mem::take(&mut self.play_when_read) {
            let command = self.toggle();
            self.send(command);
        }
    }

    /// Saves the queue, except while the library is read and the queue looks empty.
    fn save_queue(&self) {
        if !self.ready {
            return;
        }
        self.queue_file.save(self.queue_text());
    }

    /// Restores the saved queue: paused, or, after a rescan, still playing if Muse is on the
    /// same track (otherwise Muse is stopped).
    fn restore_queue(&mut self) {
        let replaying = self.replaying.take();
        let text = self.queue_text.take().unwrap_or_else(|| {
            std::fs::read_to_string(self.root.join(QUEUE_FILE)).unwrap_or_default()
        });
        let (queue, at, pos) = queue_file::decode(&text, &self.root, &self.library);
        let current = queue.get(at).map(|&t| &self.library.tracks[t].path);
        let playing = self.player.state() == MuseState::Playing;
        let plays_on = playing && replaying.is_some() && current == replaying.as_ref();
        self.player.restore(queue, at, pos, plays_on);
        self.restore_book();
        if replaying.is_some() && playing && !plays_on {
            self.send(Some(Command::Stop));
        }
    }
}

/// Lowers this thread's priority (Linux niceness is per thread), so Muse and the UI go first.
pub(crate) fn background() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}

/// What a scan finds, as it goes.
enum Found {
    /// The library as the cache had it, to show at once.
    Cached(Library),
    /// The card matched the cache: the library shown stands. The albums still needing a picture.
    Same(Vec<usize>),
    /// The library as the card has it, and the albums still needing a picture.
    Fresh(Library, Vec<usize>),
}

/// Reads the library in the background: the cache's at once, then the card's, reading what
/// changed, or every song when `full`.
fn spawn_scan(root: &Path, full: bool, progress: Arc<scan::Progress>) -> Receiver<Found> {
    let (tx, rx) = mpsc::channel();
    let root = root.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("slot-music-scan".into())
        .spawn(move || {
            background();
            let started = Instant::now();
            let shown =
                !full && scan::cached(&root).is_some_and(|lib| tx.send(Found::Cached(lib)).is_ok());
            let (lib, changed) = scan::scan(&root, full, &progress);
            let missing = covers::missing(&root, &lib);
            eprintln!(
                "slot: music: library read in {} ms",
                started.elapsed().as_millis()
            );
            let _ = tx.send(match shown && !changed {
                true => Found::Same(missing),
                false => Found::Fresh(lib, missing),
            });
        });
    if let Err(e) = spawned {
        eprintln!("slot: music: {e}");
    }
    rx
}

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shrunk_picture_is_filed_whole_under_its_album_name() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Labels/Music/Aerosmith - Get a Grip.png");
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]));
        covers::file_label(&img, &path).unwrap();
        let back = image::open(&path).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (3, 2));
        assert!(!path.with_extension("png.part").exists());
    }
}
