//! The queue: the order tracks play in, the place in it, and every edit to it. Playing it is
//! the player's.

/// Tracks in the order played, as Rockbox keeps them, and the place of the one playing. A
/// shuffle reorders the tracks once and keeps the order before it, to go back to; Repeat All
/// goes round the same order, as most players do.
#[derive(Default)]
pub struct Queue {
    tracks: Vec<usize>,
    at: usize,
    original: Option<Vec<usize>>,
}

impl Queue {
    /// `tracks` from `at`, shuffled first when there is a `seed`.
    pub fn new(mut tracks: Vec<usize>, at: usize, seed: Option<u64>) -> Queue {
        let original = seed.map(|seed| {
            let original = tracks.clone();
            deal(&mut tracks, seed);
            original
        });
        let at = at.min(tracks.len().saturating_sub(1));
        Queue {
            tracks,
            at,
            original,
        }
    }

    /// Groups (albums or artists, `sizes` long) in a shuffled order, each kept whole.
    pub fn groups(tracks: Vec<usize>, sizes: &[usize], seed: u64) -> Queue {
        let mut rest = tracks.as_slice();
        let mut groups: Vec<&[usize]> = sizes
            .iter()
            .map(|&n| {
                let (group, after) = rest.split_at(n.min(rest.len()));
                rest = after;
                group
            })
            .collect();
        deal(&mut groups, seed);
        Queue {
            tracks: groups.concat(),
            at: 0,
            original: Some(tracks),
        }
    }

    pub fn tracks(&self) -> &[usize] {
        &self.tracks
    }

    pub fn at(&self) -> usize {
        self.at
    }

    pub fn current(&self) -> Option<usize> {
        self.tracks.get(self.at).copied()
    }

    pub fn shuffled(&self) -> bool {
        self.original.is_some()
    }

    /// The place `by` from here: round the ends when `wrap`, otherwise none past them.
    pub fn step(&self, by: i32, wrap: bool) -> Option<usize> {
        let (to, n) = (self.at as i64 + by as i64, self.tracks.len() as i64);
        match wrap {
            true if n > 0 => Some(to.rem_euclid(n) as usize),
            _ if to < 0 || to >= n => None,
            _ => Some(to as usize),
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
        let current = self.current();
        match self.original.take() {
            Some(original) => {
                self.tracks = original;
                self.at = current
                    .and_then(|t| self.tracks.iter().position(|&q| q == t))
                    .unwrap_or(0);
            }
            None => {
                let Some(current) = current else {
                    return;
                };
                self.original = Some(self.tracks.clone());
                let mut rest = std::mem::take(&mut self.tracks);
                rest.remove(self.at);
                deal(&mut rest, seed);
                self.tracks = std::iter::once(current).chain(rest).collect();
                self.at = 0;
            }
        }
    }

    /// Takes out the track at `i`, except the one playing.
    pub fn remove(&mut self, i: usize) {
        if i == self.at || i >= self.tracks.len() {
            return;
        }
        let track = self.tracks.remove(i);
        if i < self.at {
            self.at -= 1;
        }
        if let Some(original) = &mut self.original {
            if let Some(p) = original.iter().position(|&t| t == track) {
                original.remove(p);
            }
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
        self.at = match self.at {
            a if a == from => to,
            a if from < a && a <= to => a - 1,
            a if to <= a && a < from => a + 1,
            a => a,
        };
    }

    /// Puts tracks just after the one playing; into an empty queue, they wait to be played.
    pub fn play_next(&mut self, tracks: &[usize]) {
        let after = (self.at + 1).min(self.tracks.len());
        self.tracks.splice(after..after, tracks.iter().copied());
        if let (Some(original), Some(current)) = (&mut self.original, self.tracks.get(self.at)) {
            let after = original
                .iter()
                .position(|t| t == current)
                .map_or(original.len(), |p| p + 1);
            original.splice(after..after, tracks.iter().copied());
        }
    }

    pub fn enqueue(&mut self, tracks: &[usize]) {
        self.tracks.extend_from_slice(tracks);
        if let Some(original) = &mut self.original {
            original.extend_from_slice(tracks);
        }
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
