mod browse;
mod covers;
mod library;
mod muse;
mod player;
pub mod queue_file;
mod screen;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

pub use browse::{
    per_page, rows, Browser, Key, Outcome, Page, Queue, Row, RowIcon, RowText, Shuffle, View,
    MENU_ROWS,
};
pub use library::{track_number, track_title, Album, Artist, Library, Track, MUSIC_DIR};
pub use muse::{parse, Command, Event, Muse, MuseState, SOCKET};
pub use player::{Player, Repeat};
pub use screen::{Legend, Screen, Status};
pub use settings::{ListSize, Settings, SETTINGS_FILE};

use covers::{Covers, Done, Job, Worker};
use queue_file::{QueueWriter, QUEUE_FILE};

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
    loading: Option<Receiver<(Library, Vec<usize>)>>,
    covers: Covers,
    browser: Browser,
    player: Player,
    settings: Settings,
    muse: Muse,
    queue_file: QueueWriter,
    connected: bool,
    shown: Option<usize>,
    art: Option<Art>,
    worker: Worker,
    /// Bumped by a rescan; older picture results are dropped.
    generation: u32,
    /// The track playing when a rescan began.
    replaying: Option<PathBuf>,
    notice: Option<(String, Instant)>,
}

impl Music {
    /// `art_px` is the size album pictures are shrunk to.
    pub fn spawn(root: &Path, art_px: u32) -> Music {
        let settings = Settings::read(root);
        let mut player = Player::default();
        player.set_repeat(settings.repeat);
        Music {
            root: root.to_path_buf(),
            library: Library::default(),
            loading: Some(scan(root)),
            covers: Covers::new(Vec::new()),
            browser: Browser::default(),
            player,
            settings,
            queue_file: QueueWriter::spawn(root),
            muse: Muse::spawn(PathBuf::from(SOCKET)),
            connected: false,
            shown: None,
            art: None,
            worker: Worker::spawn(art_px),
            generation: 0,
            replaying: None,
            notice: None,
        }
    }

    /// Reads the library again. The queue is restored by path, and the playing track plays on
    /// if it is still there.
    fn rescan(&mut self) {
        self.save_queue();
        self.replaying = match self.player.state() {
            MuseState::Playing => self
                .player
                .current()
                .map(|t| self.library.tracks[t].path.clone()),
            _ => None,
        };
        self.player.forget();
        self.browser = Browser::default();
        self.covers = Covers::new(Vec::new());
        self.shown = None;
        self.art = None;
        self.generation += 1;
        self.library = Library::default();
        self.loading = Some(scan(&self.root));
    }

    /// Once a frame. Album pictures are made only when no `game` runs.
    pub fn poll(&mut self, game: bool) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok((lib, missing)) => {
                    eprintln!(
                        "slot: music: {} artists, {} albums, {} tracks, {} to optimize",
                        lib.artists.len(),
                        lib.albums.len(),
                        lib.tracks.len(),
                        missing.len()
                    );
                    self.library = lib;
                    self.covers = Covers::new(missing);
                    self.loading = None;
                    self.restore_queue();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }
        let events: Vec<Event> = self.muse.events().collect();
        for event in events {
            match event {
                Event::Connected(up) => {
                    self.connected = up;
                    if !up {
                        self.covers.lost();
                        self.player
                            .on(&self.library, Event::State(MuseState::Stopped));
                    }
                }
                Event::Cover { base, file } => {
                    if let Some(job) = self.covers.answered(&self.root, &self.library, &base, file)
                    {
                        self.worker.send(self.generation, job);
                    }
                }
                event => {
                    let command = self.player.on(&self.library, event);
                    self.send(command);
                }
            }
        }
        if !game && self.connected {
            if let Some(command) = self.covers.next(&self.library) {
                self.muse.send(command);
            }
        }
        self.keep_queue();
        self.show_art();
    }

    /// True when the press leaves the music screen.
    pub fn press(&mut self, key: Key) -> bool {
        let lib = &self.library;
        let command = match self
            .browser
            .press(lib, self.player.queue(), &self.settings, key)
        {
            Outcome::None => None,
            Outcome::Leave => return true,
            Outcome::Rescan => {
                self.rescan();
                return false;
            }
            Outcome::Toggle => self.player.toggle(lib),
            Outcome::ClearUpNext => {
                self.player.clear_up_next();
                None
            }
            Outcome::Remove(i) => {
                self.player.remove(i);
                None
            }
            Outcome::Move(i, by) => {
                self.player.move_track(i, by);
                None
            }
            Outcome::Letter(letter) => {
                self.notice = Some((letter.to_string(), Instant::now()));
                None
            }
            Outcome::ShuffleQueue => {
                self.player.toggle_shuffle(seed());
                None
            }
            Outcome::Change(row, by) => {
                match row {
                    Row::ScreenOff => self.settings.play_screen_off ^= true,
                    Row::SkipWith => self.settings.skip_with_dpad ^= true,
                    Row::ListSize => self.settings.list_size = self.settings.list_size.step(by),
                    _ => {}
                }
                self.settings.write(&self.root);
                None
            }
            Outcome::Repeat(by) => {
                self.settings.repeat = self.settings.repeat.step(by);
                self.player.set_repeat(self.settings.repeat);
                self.settings.write(&self.root);
                None
            }
            Outcome::Seek(by) => self.player.seek(by),
            Outcome::Skip(by) => self.player.skip(lib, by),
            Outcome::Jump(to) => self.player.jump(lib, to),
            Outcome::PlayNext(tracks) => {
                self.player.play_next(&tracks);
                self.notice = Some((format!("Next +{}", tracks.len()), Instant::now()));
                None
            }
            Outcome::Enqueue(tracks) => {
                self.player.enqueue(&tracks);
                self.notice = Some((format!("+{}", tracks.len()), Instant::now()));
                None
            }
            Outcome::Play {
                tracks,
                start,
                shuffle,
            } => match shuffle {
                Shuffle::Off => self.player.play(lib, tracks, start, None),
                Shuffle::Tracks => self.player.play(lib, tracks, start, Some(seed())),
                Shuffle::Groups(sizes) => self.player.play_groups(lib, tracks, &sizes, seed()),
            },
        };
        self.send(command);
        self.keep_queue();
        false
    }

    pub fn owns_audio(&self) -> bool {
        self.player.owns_audio()
    }

    /// Whether music carries on with the screen off (`screen_off = play`).
    pub fn plays_screen_off(&self) -> bool {
        self.settings.play_screen_off
    }

    /// Whether Muse may still be sounding.
    pub fn audible(&self) -> bool {
        self.player.audible()
    }

    /// For a game starting or the screen going off.
    pub fn pause(&mut self) {
        let command = self.player.pause();
        self.send(command);
        self.keep_queue();
    }

    /// Loads the playing album's picture when the album changes or its picture is filed.
    fn show_art(&mut self) {
        let album = self.player.current().map(|t| self.library.tracks[t].album);
        if album != self.shown {
            self.shown = album;
            self.art = None;
            if let Some(album) = album {
                self.load_art(album);
            }
        }
        let done: Vec<(u32, Done)> = self.worker.done().collect();
        for (generation, done) in done {
            match done {
                _ if generation != self.generation => {}
                Done::Shrunk(album) => {
                    self.covers.shrunk();
                    if self.shown == Some(album) && self.art.is_none() {
                        self.load_art(album);
                    }
                }
                Done::Loaded(art) if self.shown == Some(art.album) => self.art = Some(art),
                Done::Loaded(_) => {}
            }
        }
    }

    fn load_art(&self, album: usize) {
        let label = covers::label(&self.root, &self.library, album);
        if label.is_file() {
            self.worker
                .send(self.generation, Job::Load { album, label });
        }
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

    /// Saves the queue, except while the library is read and the queue looks empty.
    fn save_queue(&self) {
        if self.loading.is_some() {
            return;
        }
        let (queue, at, pos) = self.player.snapshot();
        let text = queue_file::encode(&self.root, &self.library, queue, at, pos);
        self.queue_file.save(text);
    }

    /// Restores the saved queue: paused, or, after a rescan, still playing if Muse is on the
    /// same track (otherwise Muse is stopped).
    fn restore_queue(&mut self) {
        let replaying = self.replaying.take();
        let text = std::fs::read_to_string(self.root.join(QUEUE_FILE)).unwrap_or_default();
        let (queue, at, pos) = queue_file::decode(&text, &self.root, &self.library);
        let current = queue.get(at).map(|&t| &self.library.tracks[t].path);
        let playing = self.player.state() == MuseState::Playing;
        let plays_on = playing && replaying.is_some() && current == replaying.as_ref();
        self.player.restore(queue, at, pos, plays_on);
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

/// Reads the library in the background, with the albums still needing a picture.
fn scan(root: &Path) -> Receiver<(Library, Vec<usize>)> {
    let (tx, rx) = mpsc::channel();
    let root = root.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("slot-music-scan".into())
        .spawn(move || {
            background();
            let started = Instant::now();
            let lib = Library::scan(&root);
            let missing = covers::missing(&root, &lib);
            eprintln!(
                "slot: music: library read in {} ms",
                started.elapsed().as_millis()
            );
            let _ = tx.send((lib, missing));
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
