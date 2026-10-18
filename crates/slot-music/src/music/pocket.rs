//! The pocket combo: two shoulders or two triggers together, which tapped play or pause, and held
//! lock the music's other keys, so nothing changes in a pocket; the motor buzzes once to say a
//! lock and twice an unlock, as phones and earbuds do.

use std::time::{Duration, Instant};

use crate::browse::Key;

/// Whether `a` and `b` held together are the combo: the shoulders, or the triggers, in either
/// order.
pub fn is_combo(a: Key, b: Key) -> bool {
    let pair = |x, y| (a == x && b == y) || (a == y && b == x);
    pair(Key::L1, Key::R1) || pair(Key::L2, Key::R2)
}

#[derive(Default)]
pub(crate) struct PocketLock {
    /// The shoulders and triggers held, to see the combo.
    down: Vec<Key>,
    pub(crate) locked: bool,
    /// When the motor began buzzing, how many times and how long each, and how strong it was
    /// last told to be.
    buzz: Option<(Instant, u32, Duration)>,
    motor: u16,
}

impl PocketLock {
    /// `key` went down: true when, `on`, it makes the combo with one already held.
    pub fn press(&mut self, on: bool, key: Key) -> bool {
        let combo = on && self.down.iter().any(|&down| is_combo(down, key));
        self.down.push(key);
        combo
    }

    pub fn release(&mut self, key: Key) {
        self.down.retain(|&down| down != key);
    }

    /// Forgets the keys held: the screen went off, or the music screen was left.
    pub fn forget(&mut self) {
        self.down.clear();
    }

    /// Locks, or unlocks, buzzing `each` long to say so, when asked.
    pub fn toggle(&mut self, each: Option<Duration>) {
        self.locked ^= true;
        if let Some(each) = each {
            self.buzz(if self.locked { 1 } else { 2 }, each);
        }
    }

    /// Buzzes `times`, each `each` long with a gap as long after it.
    pub fn buzz(&mut self, times: u32, each: Duration) {
        self.buzz = Some((Instant::now(), times, each));
    }

    /// Whether the motor still has buzzing to do.
    pub fn buzzing(&self) -> bool {
        self.buzz.is_some()
    }

    /// How strong the motor should be, when that changes.
    pub fn rumble(&mut self) -> Option<u16> {
        let (since, times, each) = self.buzz?;
        // Each buzz is followed by a gap as long, the last too.
        let beat = since.elapsed().as_millis() / each.as_millis().max(1);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_combo_is_either_pair_in_either_order_and_only_when_on() {
        assert!(is_combo(Key::R1, Key::L1) && is_combo(Key::L2, Key::R2));
        assert!(
            !is_combo(Key::L1, Key::R2),
            "a shoulder and a trigger are not"
        );
        let mut lock = PocketLock::default();
        lock.press(false, Key::L1);
        assert!(!lock.press(false, Key::R1), "off, no combo");
    }
}
