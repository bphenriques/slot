//! The pocket combo, which the controller reads (two shoulders or two triggers together): tapped,
//! play or pause; held, lock the music's other keys, so nothing changes in a pocket; the motor buzzes once to say a
//! lock and twice an unlock, as phones and earbuds do.

use std::time::{Duration, Instant};

/// How long each buzz lasts, and the gap after it: the shortest the motor is felt at.
pub(crate) const BUZZ: Duration = Duration::from_millis(120);

#[derive(Default)]
pub(crate) struct PocketLock {
    pub(crate) locked: bool,
    /// When the motor began buzzing and how many times, and how strong it was last told to be.
    buzz: Option<(Instant, u32)>,
    motor: u16,
}

impl PocketLock {
    /// Locks, or unlocks, buzzing from `now` to say so when `buzz`: once to lock, twice to
    /// unlock.
    pub fn toggle(&mut self, buzz: bool, now: Instant) {
        self.locked ^= true;
        if buzz {
            self.buzz = Some((now, if self.locked { 1 } else { 2 }));
        }
    }

    /// Whether the motor still has buzzing to do.
    pub fn buzzing(&self) -> bool {
        self.buzz.is_some()
    }

    /// How strong the motor should be `now`, when that changes.
    pub fn rumble(&mut self, now: Instant) -> Option<u16> {
        let (since, times) = self.buzz?;
        // Each buzz is followed by a gap as long, the last too.
        let beat = now.saturating_duration_since(since).as_millis() / BUZZ.as_millis();
        if beat >= u128::from(times) * 2 {
            self.buzz = None;
        }
        let strength = match self.buzz.is_some() && beat.is_multiple_of(2) {
            true => u16::MAX,
            false => 0,
        };
        (strength != self.motor).then(|| {
            self.motor = strength;
            strength
        })
    }
}
