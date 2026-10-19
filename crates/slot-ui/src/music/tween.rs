//! How the music screens move, the one way they all do: a value eases from where it is drawn
//! to where it is going, in slot's curve, over one of these durations. A new target takes over
//! from where the value is drawn, so nothing jumps. Only what is drawn moves or fades; the state
//! it shows is already where it ends.
//!
//! `SLOT_MUSIC_SLOW` slows every one of them by as many times, to watch a move closely; 0 has
//! them arrive at once, for screenshots and tests that want the end.

use std::sync::OnceLock;

use super::slot::ease;

/// The highlight gliding to the next row.
pub const MUSIC_GLIDE_MS: u64 = 90;
/// A screen's body sliding in from the side it was opened towards; rows sliding to new places;
/// the text view moving to another line.
pub const MUSIC_SLIDE_MS: u64 = 160;
/// A new album picture fading in.
pub const MUSIC_ART_MS: u64 = 220;

/// How far along an ease `since` ms in, of one over `over` ms: 0 to 1, slot's curve.
pub fn music_eased(since: u64, over: u64) -> f32 {
    let over = over as f32 * slow();
    if over > 0.0 {
        ease((since as f32 / over).min(1.0))
    } else {
        1.0
    }
}

fn slow() -> f32 {
    static SLOW: OnceLock<f32> = OnceLock::new();
    *SLOW.get_or_init(|| {
        let slow = std::env::var("SLOT_MUSIC_SLOW").ok();
        slow.and_then(|s| s.parse().ok()).unwrap_or(1.0)
    })
}

/// A value easing from `from` to `to`, begun at `since` ms.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct MusicTween {
    pub from: f32,
    pub to: f32,
    pub since: u64,
}

impl MusicTween {
    /// Where it is, still.
    pub fn still(at: f32, now: u64) -> MusicTween {
        MusicTween {
            from: at,
            to: at,
            since: now,
        }
    }

    /// Where it is drawn `now`, easing over `over` ms.
    pub fn at(&self, now: u64, over: u64) -> f32 {
        let u = music_eased(now.saturating_sub(self.since), over);
        self.from + (self.to - self.from) * u
    }

    /// Heads for `to` from where it is drawn `now`; heading there already, it goes on.
    pub fn toward(&mut self, to: f32, now: u64, over: u64) {
        if to != self.to {
            *self = MusicTween {
                from: self.at(now, over),
                to,
                since: now,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_target_takes_over_from_where_it_is_drawn() {
        let mut t = MusicTween::still(0.0, 0);
        t.toward(10.0, 0, 100);
        let half = t.at(50, 100);
        assert!(half > 0.0 && half < 10.0);
        t.toward(20.0, 50, 100);
        assert_eq!(t.at(50, 100), half, "no jump");
        assert_eq!(t.at(150, 100), 20.0, "there in the end");
    }
}
