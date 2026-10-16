use crate::browse::QueueView;
use crate::library::Library;
use crate::muse::{Command, Event, MuseState};
use crate::queue::Queue;

/// Tracks Muse failed on in a row before the queue stops rather than skipping on.
const ERRORS_BEFORE_STOP: u32 = 3;

/// Seconds into a track, or a chapter, past which going back starts it over rather than going
/// to the one before.
const RESTART_S: f64 = 1.0;

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

/// Plays the queue through Muse: what plays, where it is, and how it repeats. Methods return
/// what to ask of Muse, if anything.
pub struct Player {
    queue: Queue,
    state: MuseState,
    reported: MuseState,
    pos: f64,
    /// When `pos` was last heard or set, to carry it on between reports.
    pos_at: Option<std::time::Instant>,
    len: f64,
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
    /// The open file's chapters, where each starts and its title; none for most songs.
    chapters: Vec<(f64, String)>,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            queue: Queue::default(),
            state: MuseState::default(),
            reported: MuseState::default(),
            pos: 0.0,
            pos_at: None,
            len: 0.0,
            errors: 0,
            repeat: Repeat::default(),
            resume_at: None,
            waiting: false,
            changed: false,
            speed: 1.0,
            chapters: Vec::new(),
        }
    }
}

impl Player {
    pub fn current(&self) -> Option<usize> {
        self.queue.current()
    }

    /// The queue as the screens read it.
    pub fn queue(&self) -> QueueView<'_> {
        QueueView {
            tracks: self.queue.tracks(),
            at: self.queue.at(),
            playing: self.state == MuseState::Playing,
            waiting: self.waiting,
            chapters: &self.chapters,
            chapter: self.chapter().map(|c| c.0),
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

    pub fn state(&self) -> MuseState {
        self.state
    }

    /// A saved queue at `pos` seconds into `at`: paused, or still `playing` after a rescan.
    pub fn restore(&mut self, queue: Vec<usize>, at: usize, pos: f64, playing: bool) {
        self.queue = Queue::new(queue, at, None);
        self.waiting = false;
        (self.pos, self.len) = (pos, 0.0);
        let any = self.queue.current().is_some();
        self.resume_at = (any && !playing).then_some(pos);
        self.state = match playing && any {
            true => MuseState::Playing,
            false => MuseState::Stopped,
        };
    }

    /// Drops the queue without stopping Muse, before a rescan renumbers the tracks; it waits to
    /// be restored.
    pub fn forget(&mut self) {
        self.waiting = self.queue.current().is_some();
        self.queue = Queue::default();
        self.resume_at = None;
    }

    /// The queue, the place in it and the seconds into that track, for saving.
    pub fn snapshot(&self) -> (&[usize], usize, f64) {
        (self.queue.tracks(), self.queue.at(), self.pos)
    }

    /// Whether a restored queue waits to be resumed.
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

    /// Seconds into the track and its length, 0 until Muse says.
    pub fn time(&self) -> (f64, f64) {
        (self.pos, self.len)
    }

    /// Seconds into the track, carried on from the last report while it plays.
    pub fn smooth_pos(&self) -> f64 {
        let since = self
            .pos_at
            .filter(|_| self.state == MuseState::Playing && self.reported == MuseState::Playing);
        let ahead = since.map_or(0.0, |at| at.elapsed().as_secs_f64());
        (self.pos + ahead).min(self.len)
    }

    /// The position in the queue, from 1, and its length.
    pub fn place(&self) -> (usize, usize) {
        (self.queue.at() + 1, self.queue.tracks().len())
    }

    /// Whether Muse has the codec, or is about to. BaseOS has no mixer, so slot keeps off it.
    pub fn owns_audio(&self) -> bool {
        self.state == MuseState::Playing
    }

    /// Whether Muse may still be sounding, until it confirms a stop or pause. The codec keeps
    /// the music's level until then.
    pub fn audible(&self) -> bool {
        self.owns_audio() || self.reported == MuseState::Playing
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Plays at `speed`, the track open too.
    pub fn set_speed(&mut self, speed: f32) -> Option<Command> {
        self.speed = speed;
        (self.state != MuseState::Stopped).then_some(Command::Speed(speed))
    }

    /// The chapter playing, from 0, with its title, start and end; none when the file has no
    /// chapters.
    pub fn chapter(&self) -> Option<(usize, &str, f64, f64)> {
        let pos = self.smooth_pos();
        let i = self.chapters.iter().rposition(|(at, _)| *at <= pos)?;
        let end = self.chapters.get(i + 1).map_or(self.len, |c| c.0);
        Some((i, &self.chapters[i].1, self.chapters[i].0, end))
    }

    /// The open file's chapters, where each starts and its title.
    pub fn chapter_list(&self) -> &[(f64, String)] {
        &self.chapters
    }

    /// Chapters known from before, until Muse tells them for the file just opened.
    pub fn set_chapters(&mut self, chapters: Vec<(f64, String)>) {
        self.chapters = chapters;
    }

    /// How many chapters the open file has.
    pub fn chapters(&self) -> usize {
        self.chapters.len()
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
        Some((std::mem::take(&mut self.queue), self.pos))
    }

    /// Puts back a queue set aside, paused where it was; Muse stops what it played instead.
    pub fn put(&mut self, queue: Queue, pos: f64) -> Option<Command> {
        self.queue = queue;
        (self.pos, self.len) = (pos, 0.0);
        self.resume_at = Some(pos);
        self.changed = true;
        self.chapters.clear();
        let stop = self.state != MuseState::Stopped;
        self.state = MuseState::Stopped;
        stop.then_some(Command::Stop)
    }

    /// Goes to the start of the open file's chapter `i`.
    pub fn go_chapter(&mut self, i: usize) -> Option<Command> {
        let at = self.chapters.get(i)?.0;
        self.seek_to(at)
    }

    /// Goes back `secs`, playing or paused.
    pub fn rewind(&mut self, secs: f64) -> Option<Command> {
        (secs > 0.0).then(|| self.seek_to(self.pos - secs))?
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
            MuseState::Playing => self.pause(),
            MuseState::Paused => {
                self.state = MuseState::Playing;
                self.changed = true;
                Some(Command::Resume)
            }
            MuseState::Stopped => self.start(lib),
        }
    }

    pub fn pause(&mut self) -> Option<Command> {
        (self.state == MuseState::Playing).then(|| {
            self.state = MuseState::Paused;
            self.changed = true;
            Command::Pause
        })
    }

    pub fn stop(&mut self) -> Option<Command> {
        self.queue = Queue::default();
        self.resume_at = None;
        self.changed = true;
        (self.state != MuseState::Stopped).then(|| {
            self.state = MuseState::Stopped;
            Command::Stop
        })
    }

    /// The track after this one: the next in the queue, or the first again under Repeat All.
    pub fn up_next(&self) -> Option<usize> {
        let to = self.queue.step(1, self.repeat == Repeat::All)?;
        (to != self.queue.at()).then(|| self.queue.tracks()[to])
    }

    /// Moves by a chapter within the file while it has them, then by a track. Repeat All wraps
    /// round the queue; otherwise a skip past an end does nothing. Back, a moment into a track
    /// or chapter, starts it over instead, as players do.
    pub fn skip(&mut self, lib: &Library, by: i32) -> Option<Command> {
        if let Some(at) = self
            .chapter_skip(by)
            .filter(|_| self.state != MuseState::Stopped)
        {
            return self.seek_to(at);
        }
        let to = self.queue.step(by, self.repeat == Repeat::All)?;
        self.queue.go(to);
        self.resume_at = None;
        self.start(lib)
    }

    /// Where a skip lands within the file: its chapter's start, the one before or the next; a
    /// file without chapters is one, from 0.
    fn chapter_skip(&self, by: i32) -> Option<f64> {
        let starts: Vec<f64> = match self.chapters.is_empty() {
            true => vec![0.0],
            false => self.chapters.iter().map(|c| c.0).collect(),
        };
        let pos = self.smooth_pos();
        let i = starts.iter().rposition(|&at| at <= pos).unwrap_or(0);
        match by < 0 {
            true if pos - starts[i] >= RESTART_S => Some(starts[i]),
            true => i.checked_sub(1).map(|j| starts[j]),
            false => starts.get(i + 1).copied(),
        }
    }

    pub fn seek(&mut self, by: i32) -> Option<Command> {
        self.seek_to(self.pos + by as f64)
    }

    /// Moves to `at` seconds into the track, short of its end.
    pub fn seek_to(&mut self, at: f64) -> Option<Command> {
        if self.state == MuseState::Stopped {
            return None;
        }
        let at = at.clamp(0.0, (self.len - 1.0).max(0.0));
        self.pos = at;
        self.pos_at = None;
        Some(Command::Seek(at))
    }

    pub fn on(&mut self, lib: &Library, event: Event) -> Option<Command> {
        match event {
            Event::State(state) => (self.state, self.reported) = (state, state),
            Event::Pos { at, len } => {
                (self.pos, self.len) = (at, len);
                self.pos_at = Some(std::time::Instant::now());
                self.errors = 0;
            }
            // An end for another track is an earlier skip arriving late.
            Event::End(path) if self.current().map(|t| &lib.tracks[t].path) == Some(&path) => {
                return match self.repeat {
                    Repeat::One => self.start(lib),
                    _ => self.next(lib),
                };
            }
            Event::Error(why) => {
                eprintln!("slot: music: {why}");
                if self.state == MuseState::Playing {
                    self.errors += 1;
                    return match self.errors < ERRORS_BEFORE_STOP {
                        true => self.next(lib),
                        false => {
                            self.state = MuseState::Stopped;
                            self.changed = true;
                            None
                        }
                    };
                }
            }
            Event::Meta => self.chapters.clear(),
            Event::Chapter { at, title } => self.chapters.push((at, title)),
            Event::End(_) | Event::Connected(_) => {}
        }
        None
    }

    /// The next track, also after a failure: Repeat One does not retry a broken track.
    fn next(&mut self, lib: &Library) -> Option<Command> {
        match self.queue.step(1, self.repeat == Repeat::All) {
            Some(_) => self.skip(lib, 1),
            None => {
                self.state = MuseState::Stopped;
                self.changed = true;
                None
            }
        }
    }

    fn start(&mut self, lib: &Library) -> Option<Command> {
        let track = lib.tracks.get(self.current()?)?;
        self.state = MuseState::Playing;
        self.changed = true;
        let at = self.resume_at.take().unwrap_or(0.0);
        (self.pos, self.len) = (at, 0.0);
        self.chapters.clear();
        Some(Command::Play {
            path: track.path.clone(),
            at,
            // Music plays as recorded; a book at its speed.
            speed: match track.book {
                Some(_) => self.speed,
                None => 1.0,
            },
        })
    }
}
