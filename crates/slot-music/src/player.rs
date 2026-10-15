use crate::browse::Queue;
use crate::library::Library;
use crate::muse::{Command, Event, MuseState};

/// Tracks Muse failed on in a row before the queue stops rather than skipping on.
const ERRORS_BEFORE_STOP: u32 = 3;

/// What a track ending does: go on and stop at the end, start over, or play it again.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    const ORDER: [Repeat; 3] = [Repeat::Off, Repeat::All, Repeat::One];

    pub fn label(self) -> &'static str {
        match self {
            Repeat::Off => "Off",
            Repeat::All => "All",
            Repeat::One => "One",
        }
    }

    pub fn parse(text: &str) -> Option<Repeat> {
        Repeat::ORDER
            .into_iter()
            .find(|r| r.label().eq_ignore_ascii_case(text))
    }

    /// The next mode, or the one before, wrapping round.
    pub fn step(self, by: i32) -> Repeat {
        let n = Repeat::ORDER.len() as i32;
        let at = Repeat::ORDER.iter().position(|&r| r == self).unwrap_or(0) as i32;
        Repeat::ORDER[(at + by).rem_euclid(n) as usize]
    }
}

/// The queue and Muse's state. Methods return what to ask of Muse, if anything.
#[derive(Default)]
pub struct Player {
    queue: Vec<usize>,
    at: usize,
    state: MuseState,
    reported: MuseState,
    pos: f64,
    len: f64,
    errors: u32,
    repeat: Repeat,
    /// Where a restored queue left off, until it is played.
    resume_at: Option<f64>,
    /// The shuffle's seed while the queue is shuffled, for a new order each Repeat All lap.
    shuffle: Option<u64>,
    /// The order before shuffling, to restore.
    original: Option<Vec<usize>>,
    /// The albums or artists a group shuffle dealt, re-dealt whole each lap. Edits drop them.
    groups: Option<Vec<Vec<usize>>>,
    /// Whether the queue changed since it was last saved.
    changed: bool,
}

impl Player {
    pub fn current(&self) -> Option<usize> {
        self.queue.get(self.at).copied()
    }

    pub fn queue(&self) -> Queue<'_> {
        Queue {
            tracks: &self.queue,
            at: self.at,
            playing: self.state == MuseState::Playing,
        }
    }

    pub fn state(&self) -> MuseState {
        self.state
    }

    /// A saved queue at `pos` seconds into `at`: paused, or still `playing` after a rescan.
    pub fn restore(&mut self, queue: Vec<usize>, at: usize, pos: f64, playing: bool) {
        self.queue = queue;
        (self.original, self.groups, self.shuffle) = (None, None, None);
        self.at = at.min(self.queue.len().saturating_sub(1));
        (self.pos, self.len) = (pos, 0.0);
        let resumes = !self.queue.is_empty() && !playing;
        self.resume_at = resumes.then_some(pos);
        self.state = match playing && !self.queue.is_empty() {
            true => MuseState::Playing,
            false => MuseState::Stopped,
        };
    }

    /// Drops the queue without stopping Muse, before a rescan renumbers the tracks.
    pub fn forget(&mut self) {
        self.queue.clear();
        self.at = 0;
        (self.original, self.groups, self.shuffle, self.resume_at) = (None, None, None, None);
    }

    /// The queue, the place in it and the seconds into that track, for saving.
    pub fn snapshot(&self) -> (&[usize], usize, f64) {
        (&self.queue, self.at, self.pos)
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
        self.original.is_some()
    }

    /// Shuffles the queue with the playing track first, or restores the original order.
    pub fn toggle_shuffle(&mut self, seed: u64) {
        let current = self.current();
        self.groups = None;
        match self.original.take() {
            Some(original) => {
                self.queue = original;
                self.at = current
                    .and_then(|t| self.queue.iter().position(|&q| q == t))
                    .unwrap_or(0);
                self.shuffle = None;
            }
            None => {
                let Some(current) = current else {
                    return;
                };
                self.original = Some(self.queue.clone());
                let mut rest = std::mem::take(&mut self.queue);
                rest.remove(self.at);
                self.shuffle = Some(shuffled(&mut rest, seed));
                self.queue = std::iter::once(current).chain(rest).collect();
                self.at = 0;
            }
        }
        self.changed = true;
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

    /// The position in the queue, from 1, and its length.
    pub fn place(&self) -> (usize, usize) {
        (self.at + 1, self.queue.len())
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

    pub fn play(
        &mut self,
        lib: &Library,
        mut tracks: Vec<usize>,
        start: usize,
        shuffle: Option<u64>,
    ) -> Option<Command> {
        self.original = shuffle.map(|_| tracks.clone());
        self.groups = None;
        self.shuffle = shuffle.map(|seed| shuffled(&mut tracks, seed));
        self.queue = tracks;
        self.at = start.min(self.queue.len().saturating_sub(1));
        self.errors = 0;
        self.resume_at = None;
        self.start(lib)
    }

    /// Plays groups (albums or artists, `sizes` long) in a shuffled order, each in its own.
    pub fn play_groups(
        &mut self,
        lib: &Library,
        tracks: Vec<usize>,
        sizes: &[usize],
        seed: u64,
    ) -> Option<Command> {
        let mut rest = tracks.as_slice();
        let groups: Vec<Vec<usize>> = sizes
            .iter()
            .map(|&n| {
                let (group, after) = rest.split_at(n.min(rest.len()));
                rest = after;
                group.to_vec()
            })
            .collect();
        self.original = Some(tracks);
        self.shuffle = Some(seed);
        self.queue = self.deal(&groups);
        self.groups = Some(groups);
        self.at = 0;
        self.errors = 0;
        self.resume_at = None;
        self.start(lib)
    }

    /// The groups in a new order, flattened.
    fn deal(&mut self, groups: &[Vec<usize>]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..groups.len()).collect();
        self.shuffle = self.shuffle.map(|seed| shuffled(&mut order, seed));
        order
            .iter()
            .flat_map(|&g| groups[g].iter().copied())
            .collect()
    }

    /// Removes a track from the queue, except the one playing.
    pub fn remove(&mut self, i: usize) {
        if i == self.at || i >= self.queue.len() {
            return;
        }
        let track = self.queue.remove(i);
        if i < self.at {
            self.at -= 1;
        }
        if let Some(original) = &mut self.original {
            if let Some(p) = original.iter().position(|&t| t == track) {
                original.remove(p);
            }
        }
        self.edited();
    }

    /// Empties what comes after the playing track.
    pub fn clear_up_next(&mut self) {
        self.queue.truncate(self.at + 1);
        (self.original, self.shuffle) = (None, None);
        self.edited();
    }

    /// Moves a track up (-1) or down (1); returns its new place.
    pub fn move_track(&mut self, i: usize, by: i32) -> usize {
        let to = i as i64 + by as i64;
        if i >= self.queue.len() || to < 0 || to >= self.queue.len() as i64 {
            return i;
        }
        let to = to as usize;
        self.queue.swap(i, to);
        self.at = match self.at {
            a if a == i => to,
            a if a == to => i,
            a => a,
        };
        self.edited();
        to
    }

    /// A hand edit: the queue is no longer whole groups.
    fn edited(&mut self) {
        self.groups = None;
        self.changed = true;
    }

    /// Plays from another place in the queue.
    pub fn jump(&mut self, lib: &Library, to: usize) -> Option<Command> {
        if to >= self.queue.len() {
            return None;
        }
        self.at = to;
        self.errors = 0;
        self.resume_at = None;
        self.start(lib)
    }

    /// Puts tracks right after the playing one; into an empty queue, they wait to be played.
    pub fn play_next(&mut self, tracks: &[usize]) {
        let after = (self.at + 1).min(self.queue.len());
        self.queue.splice(after..after, tracks.iter().copied());
        if let (Some(original), Some(current)) = (&mut self.original, self.queue.get(self.at)) {
            let after = original
                .iter()
                .position(|t| t == current)
                .map_or(original.len(), |p| p + 1);
            original.splice(after..after, tracks.iter().copied());
        }
        self.edited();
    }

    /// Adds to the end of the queue.
    pub fn enqueue(&mut self, tracks: &[usize]) {
        self.queue.extend_from_slice(tracks);
        if let Some(original) = &mut self.original {
            original.extend_from_slice(tracks);
        }
        self.edited();
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
        self.queue.clear();
        self.at = 0;
        self.resume_at = None;
        self.shuffle = None;
        self.original = None;
        self.groups = None;
        self.changed = true;
        (self.state != MuseState::Stopped).then(|| {
            self.state = MuseState::Stopped;
            Command::Stop
        })
    }

    /// Repeat All wraps round the queue; otherwise a skip past an end does nothing.
    pub fn skip(&mut self, lib: &Library, by: i32) -> Option<Command> {
        let (to, n) = (self.at as i64 + by as i64, self.queue.len() as i64);
        if let (Repeat::All, true, Some(seed)) = (self.repeat, to >= n, self.shuffle) {
            match self.groups.take() {
                Some(groups) => {
                    self.queue = self.deal(&groups);
                    self.groups = Some(groups);
                }
                None => self.shuffle = Some(shuffled(&mut self.queue, seed)),
            }
        }
        let to = match self.repeat {
            Repeat::All if n > 0 => to.rem_euclid(n),
            _ if to < 0 || to >= n => return None,
            _ => to,
        };
        self.at = to as usize;
        self.resume_at = None;
        self.start(lib)
    }

    pub fn seek(&mut self, by: i32) -> Option<Command> {
        if self.state == MuseState::Stopped {
            return None;
        }
        let at = (self.pos + by as f64).clamp(0.0, (self.len - 1.0).max(0.0));
        self.pos = at;
        Some(Command::Seek(at))
    }

    pub fn on(&mut self, lib: &Library, event: Event) -> Option<Command> {
        match event {
            Event::State(state) => (self.state, self.reported) = (state, state),
            Event::Pos { at, len } => {
                (self.pos, self.len) = (at, len);
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
            Event::End(_) | Event::Connected(_) | Event::Cover { .. } => {}
        }
        None
    }

    /// The next track, also after a failure: Repeat One does not retry a broken track.
    fn next(&mut self, lib: &Library) -> Option<Command> {
        match self.at + 1 < self.queue.len() || self.repeat == Repeat::All {
            true => self.skip(lib, 1),
            false => {
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
        Some(Command::Play {
            path: track.path.clone(),
            at,
        })
    }
}

/// Fisher-Yates over xorshift64, enough for a playlist. Returns the next seed.
fn shuffled(tracks: &mut [usize], seed: u64) -> u64 {
    let mut x = seed | 1;
    for i in (1..tracks.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        tracks.swap(i, (x % (i as u64 + 1)) as usize);
    }
    x
}
