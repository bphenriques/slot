//! slot's player: a [`Track`] brings a file to 48 kHz stereo at its speed and loudness, and a
//! thread of its own writes it to the output, with headroom, and as one side when mono.
//! The file to follow is opened ahead, so the writer goes on into it with no gap. Pausing stops
//! the writing, so the device can be let go; the moment heard is kept, and playing on starts
//! from it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::level::Leveling;
use crate::track::Track;
use crate::Status;
use crate::{Command, Event, PlayState};

/// What every file is brought to: what the device's mixer takes.
pub const RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
/// Frames written at a time: about 85 ms, so the writer wakes a dozen times a second.
const CHUNK: usize = 4096;
/// How often the player looks for an end and says where it is.
const TICK: Duration = Duration::from_millis(100);
/// How long the writer waits after the device failed before it tries again; and after a track
/// ran out with none after it, before it lets the device go.
const IDLE: Duration = Duration::from_millis(20);
const RELEASE_AFTER: Duration = Duration::from_millis(500);
/// Room left at the top, as players leave it, for a resampled peak to go over the samples': 2 dB.
const HEADROOM: f32 = 0.794_328;

/// Where the sound goes: interleaved stereo at [`RATE`], each sample -1 to 1.
pub trait Output: Send + 'static {
    /// Plays `samples`, waiting as a device does while it is full.
    fn write(&mut self, samples: &[f32]) -> Result<(), String>;
    /// How many frames written are still to be heard.
    fn delay(&mut self) -> u64;
    /// Drops what is still to be heard: a pause or a skip is heard at once.
    fn flush(&mut self);
    /// Nothing to play for a while: lets the device go, until the next write.
    fn release(&mut self);
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

/// What the engine and the writer share, and the status read from them.
#[derive(Default)]
pub(crate) struct Shared {
    /// Written while `playing`.
    tracks: Mutex<Tracks>,
    playing: AtomicBool,
    /// Frames of the file playing written since it was opened or moved, and of those, still
    /// to be heard.
    written: AtomicU64,
    queued: AtomicU64,
    /// The file ran out, with nothing after it, or went on into the next.
    ended: AtomicBool,
    went_on: AtomicBool,
    /// What is still to be heard is dropped; or, paused or stopped, the device is let go at
    /// once, for a game to take.
    flush: AtomicBool,
    release: AtomicBool,
    quit: AtomicBool,
    /// Both sides alike, for one ear or one speaker; and louder, as an f32's bits.
    mono: AtomicBool,
    boost: AtomicU32,
    /// Why the file stopped playing, or the device stopped working, to tell slot once; and
    /// whether the writer is gone, its thread panicked.
    failed: Mutex<Option<String>>,
    lost: Mutex<Option<String>>,
    dead: AtomicBool,
    /// Wakes the writer, which waits on it while there is nothing to write.
    wake: Condvar,
    /// What the engine says of the file open, and when the writer last wrote: the status is
    /// worked out from them.
    published: Mutex<Published>,
    sampled: Mutex<Option<Instant>>,
}

/// The engine's state and the file open, as it last said: where it was opened or moved to, at
/// what speed, and how long it is.
#[derive(Copy, Clone, Default)]
struct Published {
    state: PlayState,
    open: bool,
    from: f64,
    speed: f32,
    len: f64,
}

/// The player now: the moment heard is where the file was opened or moved to, with what was
/// written and heard since, and, playing, what the device has played since the last write.
pub(crate) fn status(shared: &Shared) -> Status {
    let p = *lock(&shared.published);
    if !p.open {
        return Status {
            state: p.state,
            ..Status::default()
        };
    }
    let written = shared.written.load(Ordering::Acquire);
    let queued = shared.queued.load(Ordering::Acquire);
    let mut heard = written.saturating_sub(queued) as f64;
    if p.state == PlayState::Playing && shared.playing.load(Ordering::Acquire) {
        let since = lock(&shared.sampled).map_or(0.0, |at| at.elapsed().as_secs_f64());
        heard += (since * f64::from(RATE)).min(queued as f64);
    }
    let at = p.from + heard * f64::from(p.speed) / f64::from(RATE);
    Status {
        state: p.state,
        at: if p.len > 0.0 { at.min(p.len) } else { at },
        len: p.len,
    }
}

impl Shared {
    fn wake(&self) {
        self.wake.notify_one();
    }

    fn fail(&self, why: String) {
        lock(&self.failed).get_or_insert(why);
    }

    fn lose(&self, why: String) {
        lock(&self.lost).get_or_insert(why);
    }
}

/// `mutex`, held: a thread that panicked holding it leaves it as it was.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A file open: where it was opened or moved to, the speed since, and how long it is.
struct Now {
    path: PathBuf,
    from: f64,
    speed: f32,
    len: f64,
}

/// The player, as the thread commands go to sees it.
struct Engine {
    shared: Arc<Shared>,
    now: Option<Now>,
    next: Option<Now>,
    state: PlayState,
    leveling: Leveling,
    skip_silence: bool,
}

/// Plays into `output` what `commands` asks, telling `events`, until slot drops its end.
pub fn run(
    output: impl Output,
    shared: Arc<Shared>,
    commands: &Receiver<Command>,
    events: &Sender<Event>,
) {
    shared.boost.store(1f32.to_bits(), Ordering::Release);
    let writer = {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("slot-player-out".into())
            .spawn(move || {
                // A panic here, outside a file's decoding, ends the player: slot is told, and
                // starts another.
                let out = AssertUnwindSafe(|| write(output, &shared));
                if catch_unwind(out).is_err() {
                    shared.lose("the sound stopped".into());
                    shared.dead.store(true, Ordering::Release);
                }
            })
    };
    if let Err(e) = writer {
        let _ = events.send(Event::Error(e.to_string()));
        let _ = events.send(Event::Gone);
        return;
    }
    let mut engine = Engine {
        shared: shared.clone(),
        now: None,
        next: None,
        state: PlayState::Stopped,
        leveling: Leveling::Off,
        skip_silence: false,
    };
    let mut open = true;
    while open {
        // Only playing is there an end to look for, or a place to tell: else it sleeps.
        let next = match engine.state {
            PlayState::Playing => commands.recv_timeout(TICK),
            _ => commands.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let told = match next {
            Ok(command) => engine.command(command),
            Err(RecvTimeoutError::Timeout) => Vec::new(),
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let told: Vec<Event> = told.into_iter().chain(engine.tick()).collect();
        engine.publish();
        open = told.into_iter().all(|event| events.send(event).is_ok());
        if shared.dead.load(Ordering::Acquire) {
            let _ = events.send(Event::Gone);
            break;
        }
    }
    shared.quit.store(true, Ordering::Release);
    shared.wake();
}

impl Engine {
    fn command(&mut self, command: Command) -> Vec<Event> {
        match command {
            Command::Play { path, at, speed } => self.load(path, at, speed, PlayState::Playing),
            Command::Cue { path, at, speed } => self.load(path, at, speed, PlayState::Paused),
            Command::Next(path) => {
                self.set_next(path);
                Vec::new()
            }
            Command::Pause if self.state == PlayState::Playing => {
                self.shared.playing.store(false, Ordering::Release);
                self.shared.release.store(true, Ordering::Release);
                self.move_to(None, None);
                self.go(PlayState::Paused);
                Vec::new()
            }
            Command::Resume if self.state == PlayState::Paused => {
                self.go(PlayState::Playing);
                Vec::new()
            }
            Command::Seek(at) if self.now.is_some() => {
                self.move_to(Some(at), None);
                self.flush_if_playing();
                Vec::new()
            }
            Command::Speed(speed) if self.now.is_some() => {
                self.move_to(None, Some(speed));
                self.flush_if_playing();
                Vec::new()
            }
            Command::Stop => {
                (self.now, self.next) = (None, None);
                self.put(None, true);
                self.shared.release.store(true, Ordering::Release);
                self.go(PlayState::Stopped);
                Vec::new()
            }
            Command::Mono(on) => {
                self.shared.mono.store(on, Ordering::Release);
                Vec::new()
            }
            Command::Level(leveling) => {
                self.leveling = leveling;
                let mut tracks = lock(&self.shared.tracks);
                let Tracks { now, next, .. } = &mut *tracks;
                for track in [now, next].into_iter().flatten() {
                    track.set_leveling(leveling);
                }
                Vec::new()
            }
            Command::SkipSilence(on) => {
                self.skip_silence = on;
                let mut tracks = lock(&self.shared.tracks);
                let Tracks { now, next, .. } = &mut *tracks;
                for track in [now, next].into_iter().flatten() {
                    track.set_skip_silence(on);
                }
                Vec::new()
            }
            Command::Boost(by) => {
                self.shared
                    .boost
                    .store(by.max(0.0).to_bits(), Ordering::Release);
                Vec::new()
            }
            Command::Crossfade(secs) => {
                lock(&self.shared.tracks).fade = secs.max(0.0);
                Vec::new()
            }
            Command::Pause | Command::Resume | Command::Seek(_) | Command::Speed(_) => Vec::new(),
        }
    }

    /// An end, a file gone on into the next, or a failure.
    fn tick(&mut self) -> Vec<Event> {
        if self.state != PlayState::Playing {
            return Vec::new();
        }
        // The device failing pauses where it was, so nothing is skipped for it; the file failing
        // moves slot on, and stops it after a few.
        let lost = lock(&self.shared.lost).take();
        if let Some(why) = lost {
            self.shared.release.store(true, Ordering::Release);
            self.move_to(None, None);
            self.go(PlayState::Paused);
            return vec![Event::Lost(why)];
        }
        if let Some(why) = lock(&self.shared.failed).take() {
            return vec![Event::Error(why)];
        }
        // Gone on into the next once it is heard, the last of the one before played out.
        let heard = self.shared.written.load(Ordering::Acquire)
            > self.shared.queued.load(Ordering::Acquire);
        if heard && self.shared.went_on.swap(false, Ordering::AcqRel) {
            self.now = self.next.take();
            let into = self.now.as_ref().map(|n| Event::Into(n.path.clone()));
            return into.into_iter().collect();
        }
        // Its place is the next file's until then: none to tell.
        if self.shared.went_on.load(Ordering::Acquire) {
            return Vec::new();
        }
        if self.shared.ended.swap(false, Ordering::AcqRel) {
            let path = self.now.take().map(|n| n.path);
            self.go(PlayState::Stopped);
            return path.map(Event::End).into_iter().collect();
        }
        Vec::new()
    }

    /// Says the state and the file open, for the status read from them.
    fn publish(&self) {
        let now = self.now.as_ref();
        *lock(&self.shared.published) = Published {
            state: self.state,
            open: now.is_some(),
            from: now.map_or(0.0, |n| n.from),
            speed: now.map_or(1.0, |n| n.speed),
            len: now.map_or(0.0, |n| n.len),
        };
    }

    fn go(&mut self, state: PlayState) {
        self.state = state;
        let playing = state == PlayState::Playing && self.now.is_some();
        self.shared.playing.store(playing, Ordering::Release);
        self.shared.wake();
    }

    fn flush_if_playing(&self) {
        if self.state == PlayState::Playing {
            self.shared.flush.store(true, Ordering::Release);
        }
    }

    /// Gives the writer `track`, from its start, and nothing after it, dropping what is still
    /// to be heard if `flush`.
    fn put(&mut self, track: Option<Track>, flush: bool) {
        let mut tracks = lock(&self.shared.tracks);
        (tracks.now, tracks.next, tracks.faded) = (track, None, 0);
        drop(tracks);
        self.shared.written.store(0, Ordering::Release);
        self.shared.ended.store(false, Ordering::Release);
        self.shared.went_on.store(false, Ordering::Release);
        if flush {
            self.shared.flush.store(true, Ordering::Release);
        }
    }

    /// Opens `path` from `at` to play, or, `Paused`, to wait to be played: a skip is heard at
    /// once, and a file after one that ran out follows what is left of it.
    fn load(&mut self, path: PathBuf, at: f64, speed: f32, state: PlayState) -> Vec<Event> {
        let flush = self.state == PlayState::Playing;
        self.next = None;
        match self.open(&path, at, speed) {
            Ok((track, len)) => {
                self.now = Some(Now {
                    path,
                    from: at,
                    speed,
                    len,
                });
                self.put(Some(track), flush);
                if state == PlayState::Paused {
                    self.shared.release.store(true, Ordering::Release);
                }
                self.go(state);
                Vec::new()
            }
            // The state stays: slot, still playing, moves on to the next.
            Err(why) => {
                self.now = None;
                self.put(None, true);
                self.shared.playing.store(false, Ordering::Release);
                vec![Event::Error(format!("{}: {why}", path.display()))]
            }
        }
    }

    /// `path`, opened as the player is set: its leveling and skipping silence.
    fn open(&self, path: &std::path::Path, at: f64, speed: f32) -> Result<(Track, f64), String> {
        let (mut track, len) = Track::open(path, at, speed, self.leveling)?;
        track.set_skip_silence(self.skip_silence);
        Ok((track, len))
    }

    /// Opens `path` to go on into, at the speed playing, or forgets the one opened. One that
    /// will not open is not: the file playing then ends as usual, and slot plays the next.
    fn set_next(&mut self, path: Option<PathBuf>) {
        let speed = self.now.as_ref().map_or(1.0, |n| n.speed);
        let opened = path.and_then(|path| match self.open(&path, 0.0, speed) {
            Ok((track, len)) => Some((
                track,
                Now {
                    path,
                    from: 0.0,
                    speed,
                    len,
                },
            )),
            Err(why) => {
                eprintln!("slot: player: {}: {why}", path.display());
                None
            }
        });
        let (track, now) = opened.unzip();
        lock(&self.shared.tracks).next = track;
        self.next = now;
    }

    /// Moves the file open to `at`, or else back to the moment heard, at `speed` or its own:
    /// what was decoded ahead, or written and not yet heard, plays from there. The tracks are
    /// held throughout, so the writer adds nothing between the moment and the move.
    fn move_to(&mut self, at: Option<f64>, speed: Option<f32>) {
        let shared = self.shared.clone();
        let mut tracks = lock(&shared.tracks);
        let at = at.unwrap_or_else(|| self.heard());
        let (Some(now), Some(track)) = (&mut self.now, tracks.now.as_mut()) else {
            return;
        };
        let speed = speed.unwrap_or(now.speed);
        if speed != now.speed {
            track.set_speed(speed);
            if let (Some(next), Some(upcoming)) = (tracks.next.as_mut(), self.next.as_mut()) {
                next.set_speed(speed);
                upcoming.speed = speed;
            }
        }
        let Some(track) = tracks.now.as_mut() else {
            return;
        };
        if track.seek(at).is_ok() {
            (now.from, now.speed) = (at, speed);
            shared.written.store(0, Ordering::Release);
            shared.ended.store(false, Ordering::Release);
        }
    }

    /// The moment heard: where the file was opened or moved to, and what has been heard since.
    fn heard(&self) -> f64 {
        let Some(now) = &self.now else {
            return 0.0;
        };
        let written = self.shared.written.load(Ordering::Acquire);
        let heard = written.saturating_sub(self.shared.queued.load(Ordering::Acquire));
        let at = now.from + heard as f64 * f64::from(now.speed) / f64::from(RATE);
        match now.len > 0.0 {
            true => at.min(now.len),
            false => at,
        }
    }
}

/// The writer: the file playing, a chunk at a time, while it plays; asleep otherwise, the
/// device let go when told, or when it has been quiet a while.
fn write(mut output: impl Output, shared: &Shared) {
    let mut chunk = vec![0.0; CHUNK * usize::from(CHANNELS)];
    let mut open = false;
    while !shared.quit.load(Ordering::Acquire) {
        if shared.flush.swap(false, Ordering::AcqRel) {
            output.flush();
        }
        if shared.release.swap(false, Ordering::AcqRel) {
            output.release();
            open = false;
        }
        let frames = match shared.playing.load(Ordering::Acquire) {
            true => fill(shared, &mut chunk),
            false => 0,
        };
        if frames == 0 {
            shared.queued.store(output.delay(), Ordering::Release);
            if wait(shared, open) {
                output.release();
                open = false;
            }
            continue;
        }
        // The end of the last file is filled out with silence, a frame whole.
        if let Err(why) = output.write(&chunk) {
            shared.playing.store(false, Ordering::Release);
            shared.lose(why);
            std::thread::sleep(IDLE);
        }
        shared.queued.store(output.delay(), Ordering::Release);
        *lock(&shared.sampled) = Some(Instant::now());
        open = true;
    }
}

/// Sleeps until there is something to write or do: with the device `open`, at most a while,
/// and says whether that while ran out, the device then to be let go.
fn wait(shared: &Shared, open: bool) -> bool {
    let idle = |tracks: &mut Tracks| {
        (tracks.now.is_none() || !shared.playing.load(Ordering::Acquire))
            && !shared.release.load(Ordering::Acquire)
            && !shared.quit.load(Ordering::Acquire)
    };
    let tracks = lock(&shared.tracks);
    match open {
        true => {
            let (tracks, waited) = shared
                .wake
                .wait_timeout_while(tracks, RELEASE_AFTER, idle)
                .unwrap_or_else(PoisonError::into_inner);
            drop(tracks);
            waited.timed_out()
        }
        false => {
            drop(
                shared
                    .wake
                    .wait_while(tracks, idle)
                    .unwrap_or_else(PoisonError::into_inner),
            );
            false
        }
    }
}

/// Fills `chunk` from the file playing, going on into the next where it runs out, and says how
/// many frames of it are sound; the rest is silence. A file whose decoding panics is dropped,
/// and said to have failed, rather than take the player with it.
fn fill(shared: &Shared, chunk: &mut [f32]) -> usize {
    let mut tracks = lock(&shared.tracks);
    if tracks.now.is_none() {
        return 0;
    }
    // Where in the chunk the file playing begins: past the end of one it went on from.
    let (mut n, mut begins) = (0, 0);
    while n < chunk.len() && tracks.now.is_some() {
        match catch_unwind(AssertUnwindSafe(|| tracks.read(&mut chunk[n..]))) {
            Ok(read) => n += read,
            Err(_) => {
                *tracks = Tracks::default();
                shared.fail("the file stopped decoding".into());
                return 0;
            }
        }
        if n < chunk.len() {
            // Run out: on into the next, counted from its start, or what it played as the
            // last faded out; or the end.
            tracks.now = tracks.next.take();
            match tracks.now.is_some() {
                true => {
                    shared.went_on.store(true, Ordering::Release);
                    let faded = std::mem::take(&mut tracks.faded);
                    shared.written.store(faded, Ordering::Release);
                    begins = n;
                }
                false => shared.ended.store(true, Ordering::Release),
            }
        }
    }
    chunk[n..].fill(0.0);
    let mono = shared.mono.load(Ordering::Acquire);
    let gain = HEADROOM * f32::from_bits(shared.boost.load(Ordering::Acquire));
    for frame in chunk.chunks_exact_mut(usize::from(CHANNELS)) {
        if mono {
            let both = (frame[0] + frame[1]) / 2.0;
            frame.fill(both);
        }
        frame.iter_mut().for_each(|s| *s = limit(*s * gain));
    }
    // Counted with the tracks held, so a move never meets frames from before it; what was
    // skipped as silence counts as heard.
    let channels = usize::from(CHANNELS);
    let skipped = tracks.now.as_mut().map_or(0, Track::take_skipped);
    let of_now = (n - begins).div_ceil(channels) as u64 + skipped;
    shared.written.fetch_add(of_now, Ordering::AcqRel);
    n.div_ceil(channels)
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
        let read = now.read(out);
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
            let at = 1.0 - (left - i as f64 / f64::from(RATE)) / self.fade;
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
