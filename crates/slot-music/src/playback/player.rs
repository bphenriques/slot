use std::path::PathBuf;

use crate::books::chapters::Chapter;
use crate::browse::QueueView;
use crate::library::Library;
use crate::library::TrackId;
use crate::playback::queue::Queue;
use slot_player::{Command, Event, PlayState, Status};

/// The chapter playing: its place among the file's, from 0, the chapter, and where it ends.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ChapterAt<'a> {
    pub index: usize,
    pub chapter: &'a Chapter,
    pub end: f64,
}

/// How long a place asked for is shown at most before the player's own counts again.
const ASKED_FOR: std::time::Duration = std::time::Duration::from_millis(500);

/// A place the music asked the player for: where, the length to show meanwhile, the length the
/// player had then, and since when: the first frame after, so asking needs no clock.
struct Asked {
    at: f64,
    len: f64,
    heard_len: f64,
    since: Option<std::time::Instant>,
}

/// Tracks the player failed on in a row before the queue stops rather than skipping on.
const ERRORS_BEFORE_STOP: u32 = 3;

/// Seconds into a track, or a chapter, past which going back starts it over rather than going
/// to the one before, as Spotify and Apple Music have it.
const RESTART_S: f64 = 3.0;

/// What a track ending does: go on and stop at the end, start over, or play it again.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    pub(crate) const ORDER: [Repeat; 3] = [Repeat::Off, Repeat::All, Repeat::One];

    /// Its name in `music.txt`; Now Playing shows it as a symbol.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Repeat> {
        Repeat::ORDER
            .into_iter()
            .find(|r| r.key().eq_ignore_ascii_case(text))
    }

    /// The next mode, round from One to Off.
    pub fn next(self) -> Repeat {
        crate::settings::cycle(&Repeat::ORDER, self, 0)
    }
}

/// Plays the queue through slot's player: what plays, where it is, and how it repeats. Methods return
/// what to ask of the player, if anything.
pub struct Player {
    queue: Queue,
    state: PlayState,
    /// The player as it last was, read each frame: playing or not, where, and how long.
    heard: Status,
    /// Where the music was told to open or move to, shown until the player is there.
    asked: Option<Asked>,
    errors: u32,
    repeat: Repeat,
    /// Where a restored queue left off, until it is played.
    resume_at: Option<f64>,
    /// A saved queue waits for the library to be read, to be restored.
    waiting: bool,
    /// Whether the queue changed since it was last saved.
    changed: bool,
    /// How fast a book plays, 1 as recorded; music always plays as recorded.
    speed: f32,
    /// Files' chapters, where each starts and its title, by the track they are of: the part
    /// playing's and the next's, read ahead. They count only while their track plays.
    chapters: Option<(TrackId, Vec<Chapter>)>,
    /// The file the player was told to go on into with no gap, to tell it again on a change.
    preload: Option<PathBuf>,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            queue: Queue::default(),
            state: PlayState::default(),
            heard: Status::default(),
            asked: None,
            errors: 0,
            repeat: Repeat::default(),
            resume_at: None,
            waiting: false,
            changed: false,
            speed: 1.0,
            chapters: None,
            preload: None,
        }
    }
}

impl Player {
    pub fn current(&self) -> Option<TrackId> {
        self.queue.current()
    }

    /// The queue as the screens read it.
    pub fn queue(&self) -> QueueView<'_> {
        QueueView {
            tracks: self.queue.tracks(),
            at: self.queue.at(),
            playing: self.state == PlayState::Playing,
            waiting: self.waiting,
            chapters: self.file_chapters(),
            chapter: self.chapter().map(|c| c.index),
            speed: self.speed,
        }
    }

    /// A saved queue is on the card, to be restored once the library is read.
    pub fn wait_for_saved(&mut self) {
        self.waiting = true;
    }

    /// The queue to edit; it is saved afterwards.
    pub fn edit(&mut self) -> &mut Queue {
        self.changed = true;
        &mut self.queue
    }

    pub fn state(&self) -> PlayState {
        self.state
    }

    /// A saved queue at `pos` seconds into `at`: paused, or still `playing` after a rescan.
    pub fn restore(&mut self, queue: Vec<TrackId>, at: usize, pos: f64, playing: bool) {
        self.queue = Queue::new(queue, at, None);
        self.waiting = false;
        let any = self.queue.current().is_some();
        self.resume_at = (any && !playing).then_some(pos);
        self.state = match playing && any {
            true => PlayState::Playing,
            false => PlayState::Stopped,
        };
    }

    /// Drops the queue without stopping the player, before a rescan renumbers the tracks; it waits to
    /// be restored.
    pub fn forget(&mut self) {
        self.chapters = None;
        self.waiting = self.queue.current().is_some();
        self.queue = Queue::default();
        self.resume_at = None;
    }

    /// The queue, the place in it and the seconds into that track, for saving.
    pub fn snapshot(&self) -> (&[TrackId], usize, f64) {
        (self.queue.tracks(), self.queue.at(), self.time().0)
    }

    /// Whether a restored queue waits to be resumed.
    // Kept as one of the player's primitives, as the owner chose, though only the tests ask.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn resumable(&self) -> bool {
        self.resume_at.is_some()
    }

    /// True once after each change worth saving.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn shuffled(&self) -> bool {
        self.queue.shuffled()
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    /// Seconds into the track and its length: where it was told to go until the player is
    /// there; a restored queue's place until it plays; else as the player has it.
    pub fn time(&self) -> (f64, f64) {
        if let Some(asked) = &self.asked {
            return (asked.at, asked.len);
        }
        match self.resume_at {
            Some(at) if self.heard.len == 0.0 => (at, 0.0),
            _ => (self.heard.at, self.heard.len),
        }
    }

    /// The player as it is `now`, read each frame. What it was told stands until it is there:
    /// near the place asked, a new file open, or a moment gone by.
    pub fn observe(&mut self, status: Status, now: std::time::Instant) {
        if let Some(asked) = &mut self.asked {
            let since = *asked.since.get_or_insert(now);
            let there = (status.at - asked.at).abs() < 1.0
                || status.len != asked.heard_len
                || now.saturating_duration_since(since) >= ASKED_FOR;
            if there {
                self.asked = None;
            }
        }
        // Playing on, it plays: failures in a row start counting again.
        if status.state == PlayState::Playing && status.at > 0.0 {
            self.errors = 0;
        }
        self.heard = status;
    }

    /// The player is gone, its thread failed: nothing plays.
    pub fn gone(&mut self) {
        self.state = PlayState::Stopped;
        self.heard = Status::default();
        self.asked = None;
    }

    /// Shows `at` until the player is there, the file's length `len` until it says.
    fn ask(&mut self, at: f64, len: f64) {
        self.asked = Some(Asked {
            at,
            len,
            heard_len: self.heard.len,
            since: None,
        });
    }

    /// The position in the queue, from 1, and its length.
    pub fn place(&self) -> (usize, usize) {
        (self.queue.at() + 1, self.queue.tracks().len())
    }

    /// Whether the player has the codec, or is about to. BaseOS has no mixer, so slot's own sound
    /// keeps off it.
    pub fn owns_audio(&self) -> bool {
        self.state == PlayState::Playing
    }

    /// Whether the player may still be sounding: until the device has faded out and been let
    /// go, not only told to pause. The codec keeps the music's level until then.
    pub fn audible(&self) -> bool {
        self.owns_audio() || self.heard.sounding
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Plays at `speed`, the track open too.
    pub fn set_speed(&mut self, speed: f32) -> Option<Command> {
        self.speed = speed;
        (self.state != PlayState::Stopped).then_some(Command::Speed(speed))
    }

    /// The chapter playing; none when the file has no chapters.
    pub fn chapter(&self) -> Option<ChapterAt<'_>> {
        let (pos, len) = self.time();
        let chapters = self.file_chapters();
        let index = chapters.iter().rposition(|c| c.start <= pos)?;
        let end = chapters.get(index + 1).map_or(len, |c| c.start);
        Some(ChapterAt {
            index,
            chapter: &chapters[index],
            end,
        })
    }

    /// Track `t`'s chapters, read from its file.
    pub fn set_chapters(&mut self, t: TrackId, chapters: Vec<Chapter>) {
        self.chapters = Some((t, chapters));
    }

    /// Whose chapters the player holds.
    pub fn chapters_of(&self) -> Option<TrackId> {
        self.chapters.as_ref().map(|(t, _)| *t)
    }

    /// The chapters of the file playing, when they are known: those of another count for
    /// nothing.
    fn file_chapters(&self) -> &[Chapter] {
        match &self.chapters {
            Some((t, chapters)) if Some(*t) == self.current() => chapters,
            _ => &[],
        }
    }

    /// How many chapters the open file has.
    pub fn chapters(&self) -> usize {
        self.file_chapters().len()
    }

    /// Plays a new queue from its start.
    pub fn play(&mut self, lib: &Library, queue: Queue) -> Option<Command> {
        self.queue = queue;
        self.errors = 0;
        self.resume_at = None;
        self.start(lib)
    }

    /// Takes the queue and where it is, leaving none: the music's, set aside while a book plays.
    pub fn take(&mut self) -> Option<(Queue, f64)> {
        self.queue.current()?;
        self.changed = true;
        let pos = self.time().0;
        Some((std::mem::take(&mut self.queue), pos))
    }

    /// Puts back a queue set aside, paused where it was; the player stops what it played instead.
    pub fn put(&mut self, queue: Queue, pos: f64) -> Option<Command> {
        self.queue = queue;
        self.ask(pos, 0.0);
        self.resume_at = Some(pos);
        self.changed = true;
        let stop = self.state != PlayState::Stopped;
        self.state = PlayState::Stopped;
        stop.then_some(Command::Stop)
    }

    /// Goes to the start of the open file's chapter `i`.
    pub fn go_chapter(&mut self, i: usize) -> Option<Command> {
        let at = self.file_chapters().get(i)?.start;
        self.seek_to(at)
    }

    /// Goes back `secs`, playing or paused.
    pub fn rewind(&mut self, secs: f64) -> Option<Command> {
        (secs > 0.0).then(|| self.seek_to(self.time().0 - secs))?
    }

    /// Plays a new queue from `at` seconds into its first track: a book where it was left.
    pub fn play_at(&mut self, lib: &Library, queue: Queue, at: f64) -> Option<Command> {
        self.queue = queue;
        self.errors = 0;
        self.resume_at = Some(at);
        self.start(lib)
    }

    /// Plays from another place in the queue.
    pub fn jump(&mut self, lib: &Library, to: usize) -> Option<Command> {
        if !self.queue.go(to) {
            return None;
        }
        self.errors = 0;
        self.resume_at = None;
        self.start(lib)
    }

    pub fn toggle(&mut self, lib: &Library) -> Option<Command> {
        match self.state {
            PlayState::Playing => self.pause(),
            PlayState::Paused => {
                self.state = PlayState::Playing;
                self.changed = true;
                Some(Command::Resume)
            }
            PlayState::Stopped => self.start(lib),
        }
    }

    pub fn pause(&mut self) -> Option<Command> {
        (self.state == PlayState::Playing).then(|| {
            self.state = PlayState::Paused;
            self.changed = true;
            Command::Pause
        })
    }

    /// Stops, the queue kept: a primitive kept as the owner chose, though only the tests ask.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn stop(&mut self) -> Option<Command> {
        self.queue = Queue::default();
        self.resume_at = None;
        self.preload = None;
        self.changed = true;
        (self.state != PlayState::Stopped).then(|| {
            self.state = PlayState::Stopped;
            Command::Stop
        })
    }

    /// What the player is to go on into with no gap, when `gapless`: the track that follows
    /// this one, as the queue and the repeat go; told again only when it changes.
    pub fn upcoming(&mut self, lib: &Library, gapless: bool) -> Option<Command> {
        let next = self
            .following(lib)
            .filter(|_| gapless && self.state != PlayState::Stopped)
            .map(|t| &lib.tracks[t].path);
        // Compared before it is copied: this is asked every frame.
        (next != self.preload.as_ref()).then(|| {
            self.preload = next.cloned();
            Command::Next(self.preload.clone())
        })
    }

    /// Where the queue goes when the track playing runs out: the next, round again under Repeat
    /// All, or the same under Repeat One. A book goes on to its next part only, and ends.
    fn following_at(&self, lib: &Library) -> Option<usize> {
        let book = lib.tracks.get(self.current()?.0)?.book.is_some();
        match (self.repeat, book) {
            (Repeat::One, false) => Some(self.queue.at()),
            (repeat, book) => self.queue.step(1, repeat == Repeat::All && !book),
        }
    }

    fn following(&self, lib: &Library) -> Option<TrackId> {
        self.following_at(lib).map(|at| self.queue.tracks()[at])
    }

    /// The track after this one: the next in the queue, or the first again under Repeat All.
    pub fn up_next(&self) -> Option<TrackId> {
        let to = self.queue.step(1, self.repeat == Repeat::All)?;
        (to != self.queue.at()).then(|| self.queue.tracks()[to])
    }

    /// Moves by a chapter within the file while it has them, then by a track. Repeat All wraps
    /// round the queue; otherwise a skip past an end does nothing. Back, a moment into a track
    /// or chapter, starts it over instead, as players do.
    pub fn skip(&mut self, lib: &Library, by: i32) -> Option<Command> {
        if let Some(at) = self
            .chapter_skip(by)
            .filter(|_| self.state != PlayState::Stopped)
        {
            return self.seek_to(at);
        }
        let to = self.queue.step(by, self.repeat == Repeat::All)?;
        self.queue.go(to);
        self.resume_at = None;
        // Paused, the song skipped to waits, as players have it.
        match self.state {
            PlayState::Paused => self.open(lib, PlayState::Paused),
            _ => self.start(lib),
        }
    }

    /// Where a skip lands within the file: its chapter's start, the one before or the next; a
    /// file without chapters is one, from 0.
    fn chapter_skip(&self, by: i32) -> Option<f64> {
        let chapters = self.file_chapters();
        let starts: Vec<f64> = match chapters.is_empty() {
            true => vec![0.0],
            false => chapters.iter().map(|c| c.start).collect(),
        };
        let pos = self.time().0;
        let i = starts.iter().rposition(|&at| at <= pos).unwrap_or(0);
        match by < 0 {
            true if pos - starts[i] >= RESTART_S => Some(starts[i]),
            true => i.checked_sub(1).map(|j| starts[j]),
            false => starts.get(i + 1).copied(),
        }
    }

    pub fn seek(&mut self, by: i32) -> Option<Command> {
        self.seek_to(self.time().0 + by as f64)
    }

    /// Moves to `at` seconds into the track, short of its end.
    pub fn seek_to(&mut self, at: f64) -> Option<Command> {
        if self.state == PlayState::Stopped {
            return None;
        }
        let len = self.time().1;
        let at = at.clamp(0.0, (len - 1.0).max(0.0));
        self.ask(at, len);
        Some(Command::Seek(at))
    }

    pub fn on(&mut self, lib: &Library, event: Event) -> Option<Command> {
        match event {
            // An end for another track is an earlier skip arriving late.
            Event::End(path) if self.current().map(|t| &lib.tracks[t].path) == Some(&path) => {
                return match self.repeat {
                    Repeat::One => self.start(lib),
                    _ => self.next(lib),
                };
            }
            Event::Error(why) => {
                eprintln!("slot: music: {why}");
                if self.state == PlayState::Playing {
                    self.errors += 1;
                    return match self.errors < ERRORS_BEFORE_STOP {
                        true => self.next(lib),
                        false => {
                            self.state = PlayState::Stopped;
                            self.changed = true;
                            None
                        }
                    };
                }
            }
            // Gone on with no gap into what it was told: the queue follows; into anything else,
            // a change it had not heard of yet, slot plays what it means to.
            Event::Into(path) => {
                let told = self.preload.take();
                return match (told == Some(path), self.following_at(lib)) {
                    (true, Some(at)) => {
                        self.queue.go(at);
                        self.changed = true;
                        None
                    }
                    // Nothing to play in its place: the player stops too.
                    _ => self.next(lib).or(Some(Command::Stop)),
                };
            }
            // The device stopped: the player paused where it was.
            Event::Lost(_) => self.state = PlayState::Paused,
            Event::End(_) | Event::Gone => {}
        }
        None
    }

    /// The next track, also after a failure: Repeat One does not retry a broken track.
    fn next(&mut self, lib: &Library) -> Option<Command> {
        match self.queue.step(1, self.repeat == Repeat::All) {
            Some(_) => self.skip(lib, 1),
            None => {
                self.state = PlayState::Stopped;
                self.changed = true;
                None
            }
        }
    }

    fn start(&mut self, lib: &Library) -> Option<Command> {
        self.open(lib, PlayState::Playing)
    }

    /// Opens the track at the queue's place to play, or, `Paused`, to wait to be played.
    fn open(&mut self, lib: &Library, state: PlayState) -> Option<Command> {
        let track = lib.tracks.get(self.current()?.0)?;
        self.state = state;
        self.changed = true;
        // A file played anew has nothing after it until told again.
        self.preload = None;
        let at = self.resume_at.take().unwrap_or(0.0);
        self.ask(at, 0.0);
        let path = track.path.clone();
        // Music plays as recorded; a book at its speed.
        let speed = track.book.map_or(1.0, |_| self.speed);
        Some(match state {
            PlayState::Paused => Command::Cue { path, at, speed },
            _ => Command::Play { path, at, speed },
        })
    }
}
