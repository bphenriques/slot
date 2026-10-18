//! Keys tapped, held and repeated, with the timing of slot's shelf. The screens say how each
//! key acts; this says when, knowing nothing of what the keys mean.

use std::time::{Duration, Instant};

/// How long a key is held before it repeats, then how often, quickening.
const DELAY: Duration = Duration::from_millis(400);
const EVERY: [Duration; 4] = [
    Duration::from_millis(110),
    Duration::from_millis(85),
    Duration::from_millis(65),
    Duration::from_millis(50),
];
/// How long a key is held to act as held rather than tapped.
const LONG: Duration = Duration::from_millis(500);

/// How a key acts while it is down.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    /// When pressed, once.
    Once,
    /// When pressed, then as held again and again: moving through a list.
    Repeat,
    /// Tapped, when let go; held a moment, once as held instead.
    Hold,
    /// Tapped, when let go; held, again and again as held: seeking.
    HoldRepeat,
}

/// A key acting: tapped, or held.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Press<K> {
    Tap(K),
    Held(K),
}

/// The key down, how it acts, when it next acts held and how often it has.
struct Down<K> {
    key: K,
    mode: Mode,
    next: Instant,
    fired: usize,
}

/// The key down, the last pressed: a press replaces the one before.
pub struct Keys<K> {
    down: Option<Down<K>>,
}

impl<K> Default for Keys<K> {
    fn default() -> Self {
        Keys { down: None }
    }
}

impl<K: Copy + PartialEq> Keys<K> {
    /// `key` went down, acting as `mode`: a tap now, unless it waits to see if it is held.
    pub fn press(&mut self, key: K, mode: Mode) -> Option<Press<K>> {
        let wait = match mode {
            Mode::Hold => LONG,
            _ => DELAY,
        };
        self.down = (mode != Mode::Once).then(|| Down {
            key,
            mode,
            next: Instant::now() + wait,
            fired: 0,
        });
        matches!(mode, Mode::Once | Mode::Repeat).then_some(Press::Tap(key))
    }

    /// `key` went up: a tap, if it waited for that and was not held.
    pub fn release(&mut self, key: K) -> Option<Press<K>> {
        let down = self.down.take_if(|d| d.key == key)?;
        let waited = matches!(down.mode, Mode::Hold | Mode::HoldRepeat);
        (waited && down.fired == 0).then_some(Press::Tap(key))
    }

    /// Whether a key is down, to act on as it is held.
    pub fn held(&self) -> bool {
        self.down.is_some()
    }

    /// The screen was left: the key down does nothing more.
    pub fn cancel(&mut self) {
        self.down = None;
    }

    /// How far the key down is to acting held, 0 to 1, while it waits to: for a ring that
    /// fills, so a hold can be let go before it acts.
    pub fn holding(&self) -> Option<(K, f32)> {
        let down = self
            .down
            .as_ref()
            .filter(|d| d.mode == Mode::Hold && d.fired == 0)?;
        let left = down.next.saturating_duration_since(Instant::now());
        Some((down.key, 1.0 - left.as_secs_f32() / LONG.as_secs_f32()))
    }

    /// The key down acting as held, when it is due; a hold acts once.
    pub fn due(&mut self) -> Option<Press<K>> {
        let now = Instant::now();
        let down = self
            .down
            .as_mut()
            .filter(|d| now >= d.next && !(d.mode == Mode::Hold && d.fired > 0))?;
        down.next = now + EVERY[down.fired.min(EVERY.len() - 1)];
        down.fired += 1;
        Some(Press::Held(down.key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes the key down due at once.
    fn hurry(keys: &mut Keys<char>) {
        if let Some(d) = &mut keys.down {
            d.next = Instant::now();
        }
    }

    #[test]
    fn once_and_repeat_act_when_pressed_and_hold_waits() {
        let mut keys = Keys::default();
        assert_eq!(keys.press('a', Mode::Once), Some(Press::Tap('a')));
        assert_eq!(keys.release('a'), None);
        assert_eq!(keys.press('a', Mode::Repeat), Some(Press::Tap('a')));
        assert_eq!(keys.press('b', Mode::Hold), None);
        assert_eq!(keys.release('a'), None, "b replaced a");
        assert_eq!(keys.release('b'), Some(Press::Tap('b')));
    }

    #[test]
    fn a_hold_acts_once_and_then_is_no_tap() {
        let mut keys = Keys::default();
        keys.press('a', Mode::Hold);
        assert!(
            keys.holding().is_some_and(|(k, d)| k == 'a' && d < 0.5),
            "just pressed"
        );
        assert_eq!(keys.due(), None, "not yet");
        hurry(&mut keys);
        assert_eq!(keys.due(), Some(Press::Held('a')));
        hurry(&mut keys);
        assert_eq!(keys.due(), None, "once");
        assert_eq!(keys.holding(), None, "done");
        assert_eq!(keys.release('a'), None);
    }

    #[test]
    fn a_repeat_goes_on_until_let_go() {
        let mut keys = Keys::default();
        keys.press('a', Mode::HoldRepeat);
        for _ in 0..3 {
            hurry(&mut keys);
            assert_eq!(keys.due(), Some(Press::Held('a')));
        }
        assert_eq!(keys.release('a'), None, "held, not tapped");
        hurry(&mut keys);
        assert_eq!(keys.due(), None);
    }

    #[test]
    fn leaving_the_screen_drops_the_key_down() {
        let mut keys = Keys::default();
        keys.press('a', Mode::Hold);
        keys.cancel();
        assert_eq!(keys.release('a'), None);
    }
}
