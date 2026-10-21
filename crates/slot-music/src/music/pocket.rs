//! The pocket combo, which the controller reads (any two of the shoulders and triggers together):
//! held, it locks the music's other keys, so nothing changes in a pocket; the motor buzzes long
//! once to say a lock and twice an unlock, as phones and earbuds do, and ticks short for what was
//! done unseen (a bookmark, a jump into another chapter), so the two are never taken for each other.

use std::time::{Duration, Instant};

/// How long a tick lasts, and the gap after it: the shortest the motor is felt at.
pub(crate) const BUZZ: Duration = Duration::from_millis(120);
/// How long a lock's or an unlock's buzz lasts, and the gap after: twice a tick, to tell apart.
pub(crate) const LOCK_BUZZ: Duration = Duration::from_millis(240);

#[derive(Default)]
pub(crate) struct PocketLock {
    pub(crate) locked: bool,
    /// When the motor began buzzing, how many times and how long each, and how strong it was
    /// last told to be.
    buzz: Option<(Instant, u32, Duration)>,
    motor: u16,
}

impl PocketLock {
    /// Locks, or unlocks, buzzing long from `now` to say so: once to lock, twice to unlock.
    pub fn toggle(&mut self, now: Instant) {
        self.locked ^= true;
        self.buzz = Some((now, if self.locked { 1 } else { 2 }, LOCK_BUZZ));
    }

    /// Ticks once, short, from `now`: to say what was done unseen.
    pub fn tick(&mut self, now: Instant) {
        self.buzz = Some((now, 1, BUZZ));
    }

    /// Whether the motor still has buzzing to do.
    pub fn buzzing(&self) -> bool {
        self.buzz.is_some()
    }

    /// How strong the motor should be `now`, when that changes.
    pub fn rumble(&mut self, now: Instant) -> Option<u16> {
        let (since, times, each) = self.buzz?;
        // Each buzz is followed by a gap as long, the last too.
        let beat = now.saturating_duration_since(since).as_millis() / each.as_millis();
        if beat >= u128::from(times) * 2 {
            self.buzz = None;
        }
        let strength = if self.buzz.is_some() && beat.is_multiple_of(2) {
            u16::MAX
        } else {
            0
        };
        (strength != self.motor).then(|| {
            self.motor = strength;
            strength
        })
    }
}
