//! The queue: the order tracks play in, the place in it, and every edit to it. Playing it is
//! the player's.

use crate::library::TrackId;

/// Tracks in the order played, as Rockbox keeps them, and the place of the one playing. A
/// shuffle reorders the tracks once and keeps the order before it, to go back to; Repeat All
/// goes round the same order, as most players do. What is added plays before the rest, as
/// Spotify and Apple Music have it: `added` is just past the last of it.
#[derive(Default)]
pub struct Queue {
    tracks: Vec<TrackId>,
    at: usize,
    /// Shuffled, the order before: each track's place in `tracks`, so a song queued twice is
    /// never taken for its other copy.
    original: Option<Vec<usize>>,
    added: usize,
}

impl Queue {
    /// `tracks` from `at`, shuffled first when there is a `seed`.
    pub fn new(tracks: Vec<TrackId>, at: usize, seed: Option<u64>) -> Queue {
        let mut order: Vec<usize> = (0..tracks.len()).collect();
        if let Some(seed) = seed {
            deal(&mut order, seed);
        }
        let mut queue = Queue::dealt(&tracks, order, seed.is_some());
        queue.at = at.min(queue.tracks.len().saturating_sub(1));
        queue
    }

    /// Groups (albums or artists, `sizes` long) in a shuffled order, each kept whole.
    pub fn groups(tracks: Vec<TrackId>, sizes: &[usize], seed: u64) -> Queue {
        let mut start = 0;
        let mut groups: Vec<std::ops::Range<usize>> = sizes
            .iter()
            .map(|&n| {
                let end = (start + n).min(tracks.len());
                let group = start..end;
                start = end;
                group
            })
            .collect();
        deal(&mut groups, seed);
        Queue::dealt(&tracks, groups.into_iter().flatten().collect(), true)
    }

    /// A saved queue: `original`, when it was shuffled, the order before as `Queue::original`
    /// has it; one that is not each place once is dropped.
    pub fn restored(tracks: Vec<TrackId>, at: usize, original: Option<Vec<usize>>) -> Queue {
        let mut queue = Queue::new(tracks, at, None);
        queue.original = original.filter(|o| {
            let mut sorted = o.clone();
            sorted.sort_unstable();
            sorted.iter().copied().eq(0..queue.tracks.len())
        });
        queue
    }

    /// `tracks` in `order`, one place each, from the start; the order before kept when
    /// `shuffled`.
    fn dealt(tracks: &[TrackId], order: Vec<usize>, shuffled: bool) -> Queue {
        let mut original = vec![0; order.len()];
        for (to, &from) in order.iter().enumerate() {
            original[from] = to;
        }
        Queue {
            tracks: order.iter().map(|&i| tracks[i]).collect(),
            at: 0,
            original: shuffled.then_some(original),
            added: 0,
        }
    }

    pub fn tracks(&self) -> &[TrackId] {
        &self.tracks
    }

    pub fn at(&self) -> usize {
        self.at
    }

    pub fn current(&self) -> Option<TrackId> {
        self.tracks.get(self.at).copied()
    }

    pub fn shuffled(&self) -> bool {
        self.original.is_some()
    }

    /// Shuffled, the order before, as places in `tracks`: for saving.
    pub fn original(&self) -> Option<&[usize]> {
        self.original.as_deref()
    }

    /// The place `by` from here: round the ends when `wrap`, otherwise none past them.
    pub fn step(&self, by: i32, wrap: bool) -> Option<usize> {
        let (to, n) = (self.at as i64 + by as i64, self.tracks.len() as i64);
        if wrap && n > 0 {
            Some(to.rem_euclid(n) as usize)
        } else if to < 0 || to >= n {
            None
        } else {
            Some(to as usize)
        }
    }

    /// Plays from another place; false past the end.
    pub fn go(&mut self, to: usize) -> bool {
        let ok = to < self.tracks.len();
        if ok {
            self.at = to;
        }
        ok
    }

    /// Shuffles what is not playing behind the track that is, or puts the order back.
    pub fn toggle_shuffle(&mut self, seed: u64) {
        match self.original.take() {
            Some(original) => {
                self.at = original.iter().position(|&p| p == self.at).unwrap_or(0);
                self.tracks = original.iter().map(|&p| self.tracks[p]).collect();
            }
            None => {
                if self.current().is_none() {
                    return;
                }
                let mut rest: Vec<usize> =
                    (0..self.tracks.len()).filter(|&i| i != self.at).collect();
                deal(&mut rest, seed);
                let order = std::iter::once(self.at).chain(rest).collect();
                *self = Queue::dealt(&self.tracks, order, true);
            }
        }
        self.added = 0;
    }

    /// Takes out the track at `i`, except the one playing.
    pub fn remove(&mut self, i: usize) {
        if i == self.at || i >= self.tracks.len() {
            return;
        }
        self.tracks.remove(i);
        self.added -= usize::from(i < self.added);
        if i < self.at {
            self.at -= 1;
        }
        if let Some(original) = &mut self.original {
            original.retain(|&p| p != i);
            original
                .iter_mut()
                .filter(|p| **p > i)
                .for_each(|p| *p -= 1);
        }
    }

    /// Moves the track at `from` to `to`; the one playing keeps playing wherever it lands.
    pub fn move_to(&mut self, from: usize, to: usize) {
        let n = self.tracks.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        // A track carried into what was added is added too, and one carried out of it is not.
        self.added = match self.added {
            a if from < a && a <= to => a - 1,
            a if to < a && a <= from => a + 1,
            a => a,
        };
        let moved = |p: usize| match p {
            p if p == from => to,
            p if from < p && p <= to => p - 1,
            p if to <= p && p < from => p + 1,
            p => p,
        };
        self.at = moved(self.at);
        if let Some(original) = &mut self.original {
            original.iter_mut().for_each(|p| *p = moved(*p));
        }
    }

    /// Puts `tracks` in at `at`; shuffled, the order before has them after its `after`th.
    fn insert(&mut self, at: usize, tracks: &[TrackId], after: impl Fn(&[usize]) -> usize) {
        self.tracks.splice(at..at, tracks.iter().copied());
        if let Some(original) = &mut self.original {
            original
                .iter_mut()
                .filter(|p| **p >= at)
                .for_each(|p| *p += tracks.len());
            let after = after(original);
            original.splice(after..after, at..at + tracks.len());
        }
    }

    /// Puts tracks just after the one playing, ahead of what was added before; into an empty
    /// queue, they wait to be played. The order before has them after the one playing too.
    pub fn play_next(&mut self, tracks: &[TrackId]) {
        let after = (self.at + 1).min(self.tracks.len());
        let current = self.at;
        let empty = self.tracks.is_empty();
        self.insert(after, tracks, |original| {
            original
                .iter()
                .position(|&p| p == current && !empty)
                .map_or(original.len(), |p| p + 1)
        });
        self.added = self.added.max(after) + tracks.len();
    }

    /// Takes out everything but the one playing, as Clear in a queue does.
    pub fn clear(&mut self) {
        self.tracks = self.current().into_iter().collect();
        self.at = 0;
        self.original = self
            .original
            .take()
            .map(|_| (0..self.tracks.len()).collect());
        self.added = 0;
    }

    /// Adds tracks after the one playing and what was added before it, ahead of the rest; the
    /// order before has them at its end.
    pub fn enqueue(&mut self, tracks: &[TrackId]) {
        let after = self.added.max(self.at + 1).min(self.tracks.len());
        self.insert(after, tracks, <[usize]>::len);
        self.added = after + tracks.len();
    }
}

/// Shuffles `items`: Fisher-Yates over xorshift64, enough for a playlist.
fn deal<T>(items: &mut [T], seed: u64) {
    let mut x = seed | 1;
    for i in (1..items.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        items.swap(i, (x % (i as u64 + 1)) as usize);
    }
}
