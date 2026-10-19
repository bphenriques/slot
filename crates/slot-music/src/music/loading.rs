//! Reading the library: the cache's at once, then the card's, in the background. What the old
//! library's numbers meant is carried over by path, the queue and Recently Played, and the song
//! playing plays on if it is still there.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Instant;

use super::{Effect, Music};
use crate::library::covers::{self, Covers};
use crate::library::{scan, AlbumId, Library, TrackId};
use crate::playback::queue::Queue;
use crate::playback::recent::Recent;
use crate::store::queue::SavedQueue;
use crate::store::Kept;
use slot_player::{Command, PlayState};

/// The library being read, and what is put back once it is.
pub(crate) struct Scan {
    pub(crate) rx: Option<Receiver<Found>>,
    /// A library is shown: the cache's, or the card's.
    pub(crate) ready: bool,
    /// The queue and Recently Played to put back once the library is read, by path: as the
    /// card had them, or as they were before a rescan.
    pub(crate) queue_text: Option<String>,
    pub(crate) recent_text: Option<String>,
    /// The track playing when a rescan began.
    pub(crate) replaying: Option<PathBuf>,
    /// Play was pressed while the saved queue was still being read back.
    pub(crate) play_when_read: bool,
    /// How far the scan running has got.
    pub(crate) progress: Arc<scan::Progress>,
    /// What each screen kept through a new library had its cursor on, to find it again.
    pub(crate) places: Vec<String>,
    /// When the queue was last saved, to save a song's place now and then as it plays.
    pub(crate) queue_saved: Option<Instant>,
}

/// What a scan finds, as it goes.
pub(crate) enum Found {
    /// The library as the cache had it, to show at once.
    Cached(Box<Library>),
    /// The card matched the cache: the library shown stands. The albums still needing a picture.
    Same(Vec<AlbumId>),
    /// The library as the card has it, and the albums still needing a picture.
    Fresh(Box<Library>, Vec<AlbumId>),
    /// The scan ended without a library: it is waited on no more.
    Failed,
}

impl Music {
    /// Reads the library again: what changed, or every song when `full`.
    pub(crate) fn rescan(&mut self, full: bool) {
        self.note_book();
        self.save_queue();
        self.carry_over();
        self.player.forget();
        self.books.aside = None;
        self.library = Library::default();
        self.scan.ready = false;
        self.scan.progress = Arc::new(scan::Progress::default());
        self.scan.rx = Some(spawn_scan(&self.root, full, self.scan.progress.clone()));
    }

    /// Whether the library is being read.
    pub(crate) fn scanning(&self) -> bool {
        self.scan.rx.is_some()
    }

    pub(crate) fn on_found(&mut self, found: Found) {
        match found {
            Found::Cached(lib) => self.install(*lib, Vec::new()),
            // The cache was right: the library shown stands, and its pictures can be made.
            Found::Same(missing) => {
                self.pictures.covers = Covers::new(missing);
                self.scan.rx = None;
            }
            // Nothing to show, and the queue on the card kept for a scan that reads it.
            Found::Failed => {
                log::warn!("the library could not be read");
                self.scan.ready = true;
                self.player.restore(Queue::default(), 0.0, false);
            }
            Found::Fresh(lib, missing) => {
                self.scan.rx = None;
                self.install(*lib, missing);
            }
        }
    }

    /// What the library's numbers mean, kept by path while another library takes its place:
    /// the queue, Recently Played and the song playing. The screens and pictures start afresh.
    fn carry_over(&mut self) {
        self.scan.queue_text = Some(self.queue_text());
        self.scan.recent_text = Some(self.recent.encode(self.card()));
        self.scan.replaying = match self.player.state() {
            PlayState::Playing => self.player.current().map(|t| self.path(t).to_path_buf()),
            _ => None,
        };
        self.unshow();
    }

    /// The screens and pictures, whose numbers a new library no longer means: the screens that
    /// name nothing by number stay, their cursors to be found again.
    fn unshow(&mut self) {
        let mut browser = std::mem::take(&mut self.browser);
        self.scan.places = browser.unnumber(&self.context());
        self.browser = browser;
        self.pictures = Default::default();
        self.ahead.lyrics.clear();
        self.ahead.lyrics_for = None;
        self.ahead.chapters.clear();
        self.generation += 1;
    }

    /// Shows `lib`: the first one read, or a newer one from the card in place of the cache's.
    fn install(&mut self, lib: Library, missing: Vec<AlbumId>) {
        if self.scan.ready {
            self.carry_over();
        }
        log::info!(
            "{} artists, {} albums, {} tracks, {} to optimize",
            lib.artists.len(),
            lib.albums.len(),
            lib.tracks.len(),
            missing.len()
        );
        self.library = lib;
        let recent = self.scan.recent_text.take().unwrap_or_default();
        self.recent = Recent::decode(&recent, self.card());
        self.pictures.covers = Covers::new(missing);
        self.scan.ready = true;
        self.restore_queue();
        let (mut browser, places) = (
            std::mem::take(&mut self.browser),
            std::mem::take(&mut self.scan.places),
        );
        browser.renumber(&self.context(), &places);
        self.browser = browser;
        if std::mem::take(&mut self.scan.play_when_read) {
            let command = self.toggle();
            self.send(command);
        }
    }

    /// The music's queue as the queue file has it, by path.
    fn queue_text(&self) -> String {
        self.music_snapshot().encode(self.card())
    }

    /// Saves the queue, except while the library is read and the queue looks empty, or while the
    /// queue read from the card still waits for a library to be put back in.
    pub(crate) fn save_queue(&mut self) {
        if self.scan.ready && self.scan.queue_text.is_none() {
            let text = self.queue_text();
            self.out.push(Effect::Write(SavedQueue::FILE.into(), text));
        }
    }

    /// Saves the queue when the player reports a change, and, while a song plays, its place now
    /// and then, as a book's is: a sudden power cut loses a minute at most.
    pub(crate) fn keep_queue(&mut self) {
        let now = self.now.at;
        let playing = self.player.state() == PlayState::Playing && self.book().is_none();
        let due = playing
            && self
                .scan
                .queue_saved
                .is_none_or(|at| now.saturating_duration_since(at) >= QUEUE_EVERY);
        if self.player.take_changed() || due {
            self.save_queue();
            self.scan.queue_saved = Some(now);
        }
    }

    /// Puts the queue back: paused, or, after a rescan, still playing if the player is on the
    /// same track (otherwise it is stopped).
    fn restore_queue(&mut self) {
        let replaying = self.scan.replaying.take();
        let text = self.scan.queue_text.take().unwrap_or_default();
        let saved = SavedQueue::decode(&text, self.card());
        let pos = saved.pos;
        let queue = saved.queue();
        let current = queue.current().map(|t| self.path(t).to_path_buf());
        let playing = self.player.state() == PlayState::Playing;
        let plays_on = playing && replaying.is_some() && current == replaying;
        self.player.restore(queue, pos, plays_on);
        // A book playing through a rescan plays on as music does.
        let book = replaying.as_deref().filter(|_| playing && !plays_on);
        let book_plays_on = self.restore_book(book);
        if replaying.is_some() && playing && !plays_on && !book_plays_on {
            self.send(Some(Command::Stop));
        }
    }

    fn path(&self, t: TrackId) -> &Path {
        &self.library.tracks[t].path
    }
}

/// How often a song's place is saved as it plays.
const QUEUE_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// Reads the library in the background: the cache's at once, then the card's, reading what
/// changed, or every song when `full`.
pub(crate) fn spawn_scan(
    root: &Path,
    full: bool,
    progress: Arc<scan::Progress>,
) -> Receiver<Found> {
    let (tx, rx) = mpsc::channel();
    let root: PathBuf = root.to_path_buf();
    // Asked on the frame loop, which is woken as each library is ready to show.
    let frames = std::thread::current();
    let spawned = std::thread::Builder::new()
        .name("slot-music-scan".into())
        .spawn(move || {
            crate::background();
            let started = Instant::now();
            let shown = !full
                && scan::cached(&root)
                    .is_some_and(|lib| tx.send(Found::Cached(Box::new(lib))).is_ok());
            frames.unpark();
            let (lib, changed) = scan::scan(&root, full, &progress);
            let missing = covers::missing(&root, &lib);
            log::info!("library read in {} ms", started.elapsed().as_millis());
            let _ = tx.send(if shown && !changed {
                Found::Same(missing)
            } else {
                Found::Fresh(Box::new(lib), missing)
            });
            frames.unpark();
        });
    if let Err(e) = spawned {
        log::warn!("{e}");
    }
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_read_wakes_the_frame_loop_resting() {
        let card = tempfile::tempdir().unwrap();
        let rx = spawn_scan(card.path(), true, Arc::new(scan::Progress::default()));
        let began = Instant::now();
        while rx.try_recv().is_err() {
            std::thread::park_timeout(std::time::Duration::from_secs(5));
            assert!(began.elapsed() < std::time::Duration::from_secs(2), "woken");
        }
    }
}
