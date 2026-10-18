//! slot's player, as two threads that share no state, only messages and one status:
//!
//! - The engine, on the thread slot's commands come to, opens files, which reads the card and may
//!   take a while, and hands the writer each one ready, with the rest of slot's commands.
//! - The writer owns all that plays: the files open, the output, the state, where it is. It
//!   writes a chunk at a time while it plays and takes what it was sent between chunks; it tells
//!   slot what happens, and after each chunk publishes the status, one value replaced whole,
//!   which slot reads whenever it likes.
//!
//! A [`Track`] brings a file to stereo at its speed and loudness, at its own rate where the
//! device takes it, with headroom, and as one side when mono. The file to follow is opened
//! ahead, so the writer goes on into it with no gap. Pausing fades out and keeps the device on
//! silence a few seconds before letting it go, so a pause is not heard as the speaker switching
//! off; the moment heard is kept, and playing on starts from it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::level::Leveling;
use crate::track::Track;
use crate::Status;
use crate::{Command, Event, PlayState};

/// What a file is brought to when the device will not take its own rate.
pub const RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
/// Frames written at a time: about 85 ms, so the writer wakes a dozen times a second.
const CHUNK: usize = 4096;
/// How long the device is kept after the last sound, for a file that follows to go on in.
const RELEASE_AFTER: Duration = Duration::from_millis(500);
/// How often a paused device kept on silence is given more.
const HUSH_STEP: Duration = Duration::from_millis(25);
/// Room left at the top, as players leave it, for a resampled peak to go over the samples': 2 dB.
const HEADROOM: f32 = 0.794_328;

/// Where the sound goes: interleaved stereo at the rate last set, each sample -1 to 1.
pub trait Output: Send + 'static {
    /// Plays at `rate` from the next write, letting the device go first if it was open at
    /// another; false when the device will not take it, and is to be given [`RATE`] instead.
    fn set_rate(&mut self, rate: u32) -> bool;
    /// Plays `samples`, waiting as a device does while it is full.
    fn write(&mut self, samples: &[f32]) -> Result<(), String>;
    /// How many frames written are still to be heard.
    fn delay(&mut self) -> u64;
    /// Drops what is still to be heard: a pause or a skip is heard at once.
    fn flush(&mut self);
    /// How long the device is kept on silence after a pause before it is let go: none, unless
    /// letting it go is heard.
    fn kept_paused(&self) -> Duration {
        Duration::ZERO
    }
    /// Paused, where the device is kept: fades out what is still to be heard and goes on
    /// silence until the next write or [`Output::release`].
    fn hush(&mut self) {
        self.flush();
    }
    /// Gives a hushed device a little more silence, waiting while it is full.
    fn idle(&mut self) -> Result<(), String> {
        Ok(())
    }
    /// Nothing to play for a while: lets the device go, until the next write.
    fn release(&mut self);
}

/// What the engine is told: slot's commands; from the writer, that the device would not take a
/// file's rate, the file to open again at one it does; and, from slot letting the player go,
/// to stop.
pub(crate) enum In {
    Command(Command),
    Refused {
        rate: u32,
        path: PathBuf,
        at: f64,
        speed: f32,
        next: Option<PathBuf>,
    },
    Quit,
}

/// What the writer is given: a file opened to play, or to wait paused, in place of what plays;
/// one to go on into, or none; word that the file asked for would not open; or a command for
/// what plays.
enum Do {
    Play {
        track: Track,
        opened: Opened,
        state: PlayState,
    },
    Next(Option<(Track, Opened)>),
    Unplayable,
    Command(Command),
}

/// A file open: where it was opened or moved to, the speed since, and how long it is.
struct Opened {
    path: PathBuf,
    from: f64,
    speed: f32,
    len: f64,
}

/// The writer's last word on the player, replaced whole and never in part, so a reader never
/// sees half of one and half of the next.
#[derive(Copy, Clone, Default)]
struct Snapshot {
    state: PlayState,
    /// Whether a file is open; and where it was opened or moved to, at what speed, how long it
    /// is and the rate it plays at.
    open: bool,
    from: f64,
    speed: f32,
    len: f64,
    rate: u32,
    /// Of what was written since `from`, the frames heard and those still in the device, when
    /// last `sampled`, just after a write.
    heard: u64,
    queued: u64,
    sampled: Option<Instant>,
    sounding: bool,
}

impl Snapshot {
    /// Seconds into the file at `now`: where it was opened or moved to, what was heard by the
    /// last write, and, playing, what the device has played of the rest since.
    fn at(&self, now: Instant) -> f64 {
        let rate = f64::from(self.rate.max(1));
        let mut heard = self.heard as f64;
        if self.state == PlayState::Playing {
            let since = self
                .sampled
                .map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f64());
            heard += (since * rate).min(self.queued as f64);
        }
        let at = self.from + heard * f64::from(self.speed) / rate;
        match self.len > 0.0 {
            true => at.min(self.len),
            false => at,
        }
    }
}

/// The status, as the writer last published it.
#[derive(Default)]
pub(crate) struct Shared(Mutex<Snapshot>);

pub(crate) fn status(shared: &Shared) -> Status {
    let s = *lock(&shared.0);
    Status {
        state: s.state,
        at: if s.open { s.at(Instant::now()) } else { 0.0 },
        len: if s.open { s.len } else { 0.0 },
        sounding: s.sounding,
    }
}

/// `mutex`, held: a thread that panicked holding it leaves it as it was.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Plays into `output` what `inbox` asks, telling `events`, until slot lets the player go.
/// `back` is the inbox's other end, for the writer to say what the device would not take.
pub(crate) fn run(
    output: impl Output,
    shared: Arc<Shared>,
    inbox: &Receiver<In>,
    back: Sender<In>,
    events: &Sender<Event>,
) {
    let (writer, todo) = mpsc::channel();
    let told = events.clone();
    let spawned = std::thread::Builder::new()
        .name("slot-player-out".into())
        .spawn(move || {
            // A panic here, outside a file's decoding, ends the player: slot is told, and
            // starts another.
            let gone = told.clone();
            let write = AssertUnwindSafe(|| Writer::new(output, shared, told, back).run(&todo));
            if catch_unwind(write).is_err() {
                let _ = gone.send(Event::Lost("the sound stopped".into()));
                let _ = gone.send(Event::Gone);
            }
        });
    if let Err(e) = spawned {
        let _ = events.send(Event::Error(e.to_string()));
        let _ = events.send(Event::Gone);
        return;
    }
    let mut engine = Engine {
        writer,
        events: events.clone(),
        leveling: Leveling::Off,
        skip_silence: false,
        rate: RATE,
        speed: 1.0,
        refused: Vec::new(),
    };
    for message in inbox {
        if matches!(message, In::Quit) {
            break;
        }
        // The writer gone, the player is.
        if !engine.take(message) {
            let _ = events.send(Event::Gone);
            break;
        }
    }
}

/// The engine: opens the files the writer is to play, as the player is set, and passes it the
/// rest of slot's commands.
struct Engine {
    writer: Sender<Do>,
    events: Sender<Event>,
    leveling: Leveling,
    skip_silence: bool,
    /// The rate and speed of the file last given to play, which one to go on into takes; and the
    /// rates the device would not take.
    rate: u32,
    speed: f32,
    refused: Vec<u32>,
}

impl Engine {
    /// Acts on `message`; false once the writer is gone.
    fn take(&mut self, message: In) -> bool {
        let todo = match message {
            In::Command(Command::Play { path, at, speed }) => {
                self.load(path, at, speed, PlayState::Playing)
            }
            In::Command(Command::Cue { path, at, speed }) => {
                self.load(path, at, speed, PlayState::Paused)
            }
            In::Command(Command::Next(path)) => Do::Next(path.and_then(|p| self.open_next(p))),
            In::Command(command) => {
                match command {
                    Command::Speed(speed) => self.speed = speed,
                    Command::Level(leveling) => self.leveling = leveling,
                    Command::SkipSilence(on) => self.skip_silence = on,
                    _ => {}
                }
                Do::Command(command)
            }
            // Nothing of it was written: it plays on at a rate the device takes, from where it
            // was, the file after it with it.
            In::Refused {
                rate,
                path,
                at,
                speed,
                next,
            } => {
                self.refused.push(rate);
                let play = self.load(path, at, speed, PlayState::Playing);
                if self.writer.send(play).is_err() {
                    return false;
                }
                Do::Next(next.and_then(|p| self.open_next(p)))
            }
            In::Quit => return true,
        };
        self.writer.send(todo).is_ok()
    }

    /// `path` opened from `at` to play, or, `Paused`, to wait to be played; or, if it will not
    /// open, word of that, slot told why.
    fn load(&mut self, path: PathBuf, at: f64, speed: f32, state: PlayState) -> Do {
        match self.open(&path, at, speed, None) {
            Ok((track, len)) => {
                (self.rate, self.speed) = (track.rate(), speed);
                let opened = Opened {
                    path,
                    from: at,
                    speed,
                    len,
                };
                Do::Play {
                    track,
                    opened,
                    state,
                }
            }
            Err(why) => {
                let _ = self
                    .events
                    .send(Event::Error(format!("{}: {why}", path.display())));
                Do::Unplayable
            }
        }
    }

    /// `path` opened to go on into, at the speed and rate playing. One that will not open is
    /// not: the file playing then ends as usual, and slot plays the next.
    fn open_next(&self, path: PathBuf) -> Option<(Track, Opened)> {
        match self.open(&path, 0.0, self.speed, Some(self.rate)) {
            Ok((track, len)) => Some((
                track,
                Opened {
                    path,
                    from: 0.0,
                    speed: self.speed,
                    len,
                },
            )),
            Err(why) => {
                eprintln!("slot: player: {}: {why}", path.display());
                None
            }
        }
    }

    /// `path`, opened as the player is set: its leveling and skipping silence; at `rate`, or else
    /// its own unless the device would not take that.
    fn open(
        &self,
        path: &Path,
        at: f64,
        speed: f32,
        rate: Option<u32>,
    ) -> Result<(Track, f64), String> {
        let refused = &self.refused;
        let rate = |own: u32| match rate {
            Some(rate) => rate,
            None if refused.contains(&own) => RATE,
            None => own,
        };
        let (mut track, len) = Track::open(path, at, speed, self.leveling, rate)?;
        track.set_skip_silence(self.skip_silence);
        Ok((track, len))
    }
}

/// The file playing, and the one to go on into when it runs out: faded into over the last
/// `fade` seconds, if any, `faded` frames of it played so far.
#[derive(Default)]
struct Tracks {
    now: Option<Track>,
    next: Option<Track>,
    fade: f64,
    faded: u64,
    mix: Vec<f32>,
}

/// The writer, and all that plays: only it touches any of this.
struct Writer<O: Output> {
    output: O,
    shared: Arc<Shared>,
    events: Sender<Event>,
    engine: Sender<In>,
    tracks: Tracks,
    /// The files open, as `tracks` holds them; and, gone on into the next and it not yet heard,
    /// the length of the one before, which plays out of the device meanwhile.
    now: Option<Opened>,
    next: Option<Opened>,
    ending: Option<f64>,
    /// The file that ran out with none after it, told once the last of it is written.
    ended: Option<PathBuf>,
    state: PlayState,
    mono: bool,
    boost: f32,
    /// Frames of the file playing written since it was opened or moved; and of all written,
    /// those still in the device, as of `sampled`.
    written: u64,
    queued: u64,
    sampled: Option<Instant>,
    /// The rate the output was last set to, and whether it has been written to since it was let
    /// go.
    rate: u32,
    sounding: bool,
    /// Paused, the device going on silence until then.
    hushed: Option<Instant>,
    chunk: Vec<f32>,
}

impl<O: Output> Writer<O> {
    fn new(output: O, shared: Arc<Shared>, events: Sender<Event>, engine: Sender<In>) -> Self {
        Writer {
            output,
            shared,
            events,
            engine,
            tracks: Tracks::default(),
            now: None,
            next: None,
            ending: None,
            ended: None,
            state: PlayState::Stopped,
            mono: false,
            boost: 1.0,
            written: 0,
            queued: 0,
            sampled: None,
            rate: 0,
            sounding: false,
            hushed: None,
            chunk: vec![0.0; CHUNK * usize::from(CHANNELS)],
        }
    }

    /// Playing, a chunk at a time, taking what came between chunks; else asleep until something
    /// comes, the device let go once it has been quiet a while. Ends with the engine.
    fn run(&mut self, todo: &Receiver<Do>) {
        loop {
            let playing = self.playing();
            let got = match (playing, self.sounding) {
                (true, _) => todo.try_recv().map_err(|e| match e {
                    TryRecvError::Empty => RecvTimeoutError::Timeout,
                    TryRecvError::Disconnected => RecvTimeoutError::Disconnected,
                }),
                (false, true) => todo.recv_timeout(match self.hushed {
                    Some(_) => HUSH_STEP,
                    None => RELEASE_AFTER,
                }),
                (false, false) => todo.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match got {
                Ok(first) => {
                    self.take(first);
                    while let Ok(more) = todo.try_recv() {
                        self.take(more);
                    }
                }
                Err(RecvTimeoutError::Timeout) if !playing => self.rest(),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.playing() {
                self.write();
            }
            *lock(&self.shared.0) = self.snapshot();
        }
    }

    fn playing(&self) -> bool {
        self.state == PlayState::Playing && self.tracks.now.is_some()
    }

    fn take(&mut self, todo: Do) {
        match todo {
            // A skip is heard at once; a file after one that ran out follows what is left of it.
            Do::Play {
                track,
                opened,
                state,
            } => {
                if self.state == PlayState::Playing {
                    self.output.flush();
                }
                self.tracks = Tracks {
                    now: Some(track),
                    fade: self.tracks.fade,
                    ..Tracks::default()
                };
                (self.now, self.next, self.ending) = (Some(opened), None, None);
                self.recount();
                self.state = state;
                if state == PlayState::Paused {
                    self.release();
                }
            }
            Do::Next(next) => {
                let (track, opened) = next.unzip();
                (self.tracks.next, self.next) = (track, opened);
            }
            // The state stays: slot, still playing, moves on to the next.
            Do::Unplayable => {
                self.output.flush();
                (self.tracks.now, self.tracks.next) = (None, None);
                (self.now, self.next, self.ending) = (None, None, None);
                self.recount();
            }
            Do::Command(command) => self.command(command),
        }
    }

    fn command(&mut self, command: Command) {
        let open = self.now.is_some();
        match command {
            Command::Pause if self.state == PlayState::Playing => {
                self.tell_into();
                let at = self.place();
                self.hush();
                self.move_to(at, None);
                self.state = PlayState::Paused;
            }
            Command::Resume if self.state == PlayState::Paused => self.state = PlayState::Playing,
            Command::Seek(at) if open => {
                self.tell_into();
                self.move_to(at, None);
                self.flush_if_playing();
            }
            Command::Speed(speed) if open => {
                self.tell_into();
                let at = self.place();
                self.move_to(at, Some(speed));
                self.flush_if_playing();
            }
            Command::Stop => {
                (self.tracks.now, self.tracks.next) = (None, None);
                (self.now, self.next, self.ending) = (None, None, None);
                self.release();
                self.recount();
                self.state = PlayState::Stopped;
            }
            Command::Mono(on) => self.mono = on,
            Command::Boost(by) => self.boost = by.max(0.0),
            Command::Crossfade(secs) => self.tracks.fade = secs.max(0.0),
            Command::Level(leveling) => {
                let Tracks { now, next, .. } = &mut self.tracks;
                for track in [now, next].into_iter().flatten() {
                    track.set_leveling(leveling);
                }
            }
            Command::SkipSilence(on) => {
                let Tracks { now, next, .. } = &mut self.tracks;
                for track in [now, next].into_iter().flatten() {
                    track.set_skip_silence(on);
                }
            }
            // Played anew or gone on into, the engine opens them first; the rest do nothing as
            // things stand.
            Command::Play { .. }
            | Command::Cue { .. }
            | Command::Next(_)
            | Command::Pause
            | Command::Resume
            | Command::Seek(_)
            | Command::Speed(_) => {}
        }
    }

    /// The moment heard of the file playing, now.
    fn place(&self) -> f64 {
        self.counted().at(Instant::now())
    }

    /// Moves the file open to `at`, at `speed` or its own: what was decoded ahead plays from
    /// there, counted afresh.
    fn move_to(&mut self, at: f64, speed: Option<f32>) {
        let (Some(now), Some(track)) = (self.now.as_mut(), self.tracks.now.as_mut()) else {
            return;
        };
        let speed = speed.unwrap_or(now.speed);
        if speed != now.speed {
            track.set_speed(speed);
            if let (Some(next), Some(opened)) = (self.tracks.next.as_mut(), self.next.as_mut()) {
                next.set_speed(speed);
                opened.speed = speed;
            }
        }
        if track.seek(at).is_ok() {
            (now.from, now.speed) = (at, speed);
            self.recount();
        }
    }

    fn flush_if_playing(&mut self) {
        if self.state == PlayState::Playing {
            self.output.flush();
            self.recount();
        }
    }

    /// Nothing written since the file was opened or moved, nothing in the device of it.
    fn recount(&mut self) {
        (self.written, self.queued, self.sampled) = (0, 0, None);
    }

    fn release(&mut self) {
        self.output.release();
        (self.sounding, self.queued, self.sampled, self.hushed) = (false, 0, None, None);
    }

    /// Paused: what was to be heard fades out, and the device, if open and one to keep, goes on
    /// silence a while; else it is let go.
    fn hush(&mut self) {
        let keep = self.output.kept_paused();
        if !self.sounding || keep.is_zero() {
            return self.release();
        }
        self.output.hush();
        self.hushed = Some(Instant::now() + keep);
        (self.queued, self.sampled) = (0, None);
    }

    /// Nothing came while nothing plays: a hushed device is given more silence until it has
    /// been paused a while, and then, as any other, let go.
    fn rest(&mut self) {
        match self.hushed {
            Some(until) if Instant::now() < until => {
                if self.output.idle().is_err() {
                    self.release();
                }
            }
            Some(_) => {
                eprintln!("slot: player: let go, paused a while");
                self.release();
            }
            None => self.release(),
        }
    }

    /// Says the file gone on into is playing, if it was not yet heard: it is about to be moved,
    /// paused or changed, and the one before is cut short.
    fn tell_into(&mut self) {
        if self.ending.take().is_some() {
            if let Some(now) = &self.now {
                let _ = self.events.send(Event::Into(now.path.clone()));
            }
        }
    }

    /// A chunk of the file playing, written: at a rate the device was asked for first.
    fn write(&mut self) {
        let rate = self.tracks.now.as_ref().map_or(self.rate, Track::rate);
        if rate != self.rate {
            if !self.output.set_rate(rate) {
                self.refused(rate);
                return;
            }
            self.rate = rate;
        }
        self.hushed = None;
        let of_now = self.fill();
        let written = self.output.write(&self.chunk);
        if let Some(path) = self.ended.take() {
            let _ = self.events.send(Event::End(path));
        }
        if let Err(why) = written {
            // The device failing pauses where it was, so nothing is skipped for it.
            let at = self.place();
            self.release();
            self.move_to(at, None);
            self.state = PlayState::Paused;
            let _ = self.events.send(Event::Lost(why));
            return;
        }
        // Counted once written, with what the device holds after it, so what is heard is never
        // ahead of what it has.
        self.written += of_now;
        self.queued = self.output.delay();
        self.sampled = Some(Instant::now());
        self.sounding = true;
        // The file gone on into is heard: the last of the one before has played out.
        if self.ending.is_some() && self.written > self.queued {
            self.tell_into();
        }
    }

    /// The device would not take `rate`, and nothing was written at it: the engine opens the file
    /// again at one it does, and nothing plays until then.
    fn refused(&mut self, rate: u32) {
        let Some(now) = &self.now else {
            return;
        };
        let _ = self.engine.send(In::Refused {
            rate,
            path: now.path.clone(),
            at: now.from,
            speed: now.speed,
            next: self.next.as_ref().map(|n| n.path.clone()),
        });
        (self.tracks.now, self.tracks.next, self.next) = (None, None, None);
    }

    /// Fills the chunk from the file playing, going on into the next where it runs out, and says
    /// how many frames of it are the file playing's, by then; the rest is silence. A file whose
    /// decoding panics is dropped, and said to have failed, rather than take the player with it.
    fn fill(&mut self) -> u64 {
        let channels = usize::from(CHANNELS);
        // Where in the chunk the file playing begins: past the end of one it went on from.
        let (mut n, mut begins) = (0, 0);
        while n < self.chunk.len() && self.tracks.now.is_some() {
            let (tracks, chunk) = (&mut self.tracks, &mut self.chunk);
            match catch_unwind(AssertUnwindSafe(|| tracks.read(&mut chunk[n..]))) {
                Ok(read) => n += read,
                Err(_) => {
                    (self.tracks.now, self.tracks.next) = (None, None);
                    (self.now, self.next, self.ending) = (None, None, None);
                    let failed = Event::Error("the file stopped decoding".into());
                    let _ = self.events.send(failed);
                    break;
                }
            }
            if n < self.chunk.len() && self.tracks.now.is_some() {
                self.ran_out();
                begins = n;
            }
        }
        self.chunk[n..].fill(0.0);
        let gain = HEADROOM * self.boost;
        for frame in self.chunk.chunks_exact_mut(channels) {
            if self.mono {
                let both = (frame[0] + frame[1]) / 2.0;
                frame.fill(both);
            }
            frame.iter_mut().for_each(|s| *s = limit(*s * gain));
        }
        // What was skipped as silence counts as heard.
        let skipped = self.tracks.now.as_mut().map_or(0, Track::take_skipped);
        (n - begins).div_ceil(channels) as u64 + skipped
    }

    /// The file playing ran out: on into the next, counted from its start, or what it played as
    /// the last faded out; or the end, told once the chunk is written.
    fn ran_out(&mut self) {
        match self.tracks.next.take() {
            Some(next) => {
                self.tracks.now = Some(next);
                let before = std::mem::replace(&mut self.now, self.next.take());
                self.ending = before.map(|b| b.len);
                self.written = std::mem::take(&mut self.tracks.faded);
            }
            None => {
                self.tracks.now = None;
                self.state = PlayState::Stopped;
                self.ended = self.now.take().map(|now| now.path);
            }
        }
    }

    /// The status of the file playing, as counted.
    fn counted(&self) -> Snapshot {
        let rate = self.tracks.now.as_ref().map_or(self.rate, Track::rate);
        match &self.now {
            Some(now) => Snapshot {
                state: self.state,
                open: true,
                from: now.from,
                speed: now.speed,
                len: now.len,
                rate,
                heard: self.written.saturating_sub(self.queued),
                queued: self.queued,
                sampled: self.sampled,
                sounding: self.sounding,
            },
            None => Snapshot {
                state: self.state,
                sounding: self.sounding,
                ..Snapshot::default()
            },
        }
    }

    /// The status to publish: as counted; or, while the one before plays out of the device, that
    /// one at its end.
    fn snapshot(&self) -> Snapshot {
        match self.ending {
            Some(len) => Snapshot {
                from: len,
                len,
                speed: 1.0,
                heard: 0,
                queued: 0,
                sampled: None,
                ..self.counted()
            },
            None => self.counted(),
        }
    }
}

/// Rounds off what a boost would take past full scale, rather than clip it; leaves the rest.
fn limit(sample: f32) -> f32 {
    const KNEE: f32 = 0.9;
    match sample.abs() > KNEE {
        true => {
            sample.signum() * (KNEE + (1.0 - KNEE) * ((sample.abs() - KNEE) / (1.0 - KNEE)).tanh())
        }
        false => sample,
    }
}

impl Tracks {
    /// Reads the file playing into `out`; in its last `fade` seconds, mixed with the start of
    /// the next, one going down as the other comes up, the sum as loud throughout.
    fn read(&mut self, out: &mut [f32]) -> usize {
        let Some(now) = self.now.as_mut() else {
            return 0;
        };
        let left = now
            .left()
            .filter(|&left| self.fade > 0.0 && left < self.fade);
        let (read, rate) = (now.read(out), f64::from(now.rate()));
        let (Some(left), Some(next)) = (left, self.next.as_mut()) else {
            return read;
        };
        self.mix.resize(read, 0.0);
        let got = next.read(&mut self.mix);
        self.faded += (got / usize::from(CHANNELS)) as u64;
        for (i, frame) in out[..read]
            .chunks_exact_mut(usize::from(CHANNELS))
            .enumerate()
        {
            let at = 1.0 - (left - i as f64 / rate) / self.fade;
            let t = at.clamp(0.0, 1.0) as f32 * std::f32::consts::FRAC_PI_2;
            for (c, sample) in frame.iter_mut().enumerate() {
                let theirs = self
                    .mix
                    .get(i * usize::from(CHANNELS) + c)
                    .copied()
                    .unwrap_or(0.0);
                *sample = *sample * t.cos() + theirs * t.sin();
            }
        }
        read
    }
}
