//! The volume fading in on resume, and the sleep timer that fades the music out.

use std::time::{Duration, Instant};

use crate::settings::cycle;

/// How long a resume fades in, and the sleep timer's last, slower fade.
pub const FADE: Duration = Duration::from_millis(300);
pub const SLEEP_FADE: Duration = Duration::from_secs(8);

/// Which way a fade goes: in as music plays, or out to a pause as the sleep timer runs out.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Fading {
    In,
    Asleep,
}

/// A volume ramp from `from` to full, or to silence, 0 silent to 1 full.
#[derive(Copy, Clone)]
pub struct Fade {
    from: f32,
    pub fading: Fading,
    since: Instant,
    over: Duration,
}

impl Fade {
    pub fn new(from: f32, fading: Fading, over: Duration) -> Fade {
        Fade {
            from,
            fading,
            since: Instant::now(),
            over,
        }
    }

    /// A fade down to silence, which pauses at its end.
    pub fn is_out(&self) -> bool {
        self.fading != Fading::In
    }

    pub fn done(&self) -> bool {
        self.since.elapsed() >= self.over
    }

    pub fn level(&self) -> f32 {
        let u = self.since.elapsed().as_secs_f32() / self.over.as_secs_f32();
        let to = if self.is_out() { 0.0 } else { 1.0 };
        self.from + (to - self.from) * u.min(1.0)
    }
}

/// What the sleep timer waits for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sleep {
    /// Minutes, rounded up.
    Minutes(u32),
    /// The end of the song playing, or of the audiobook's chapter.
    End,
}

/// Pauses after a chosen number of minutes, or at the end of the song or the book's chapter.
#[derive(Default)]
pub struct SleepTimer {
    at: Option<Instant>,
    end: bool,
    /// The choice last made, to start again when its fade is turned back.
    chosen: Option<Sleep>,
}

impl SleepTimer {
    const CHOICES: [Option<Sleep>; 9] = [
        None,
        Some(Sleep::End),
        Some(Sleep::Minutes(5)),
        Some(Sleep::Minutes(10)),
        Some(Sleep::Minutes(15)),
        Some(Sleep::Minutes(30)),
        Some(Sleep::Minutes(45)),
        Some(Sleep::Minutes(60)),
        Some(Sleep::Minutes(120)),
    ];

    /// What it waits for, with the minutes left; `None` when off.
    pub fn left(&self) -> Option<Sleep> {
        if self.end {
            return Some(Sleep::End);
        }
        let left = self.remaining()?;
        Some(Sleep::Minutes(left.as_secs().div_ceil(60) as u32))
    }

    /// The time left, to count down; none when off or waiting for an end.
    pub fn remaining(&self) -> Option<Duration> {
        Some(self.at?.saturating_duration_since(Instant::now()))
    }

    /// Steps from the choice at or above the minutes left, as other settings step.
    pub fn step(&mut self, by: i32) {
        let choices = SleepTimer::CHOICES;
        let shown = match self.left() {
            Some(Sleep::Minutes(left)) => choices.iter().copied().find(|c| match c {
                Some(Sleep::Minutes(m)) => *m >= left,
                _ => false,
            }),
            left => Some(left),
        };
        let to = cycle(&choices, shown.flatten(), by);
        self.chosen = to;
        self.set(to);
    }

    /// Starts again as last chosen: the fade it ended in was turned back, so more is wanted,
    /// as Smart AudioBook Player has it; at an end, the next one.
    pub fn again(&mut self) {
        self.set(self.chosen);
    }

    fn set(&mut self, to: Option<Sleep>) {
        self.end = to == Some(Sleep::End);
        self.at = match to {
            Some(Sleep::Minutes(m)) => {
                Some(Instant::now() + Duration::from_secs(60 * u64::from(m)))
            }
            _ => None,
        };
    }

    /// True once, when the time is up; the timer is then off.
    pub fn due(&mut self) -> bool {
        let due = self.at.is_some_and(|at| Instant::now() >= at);
        if due {
            self.at = None;
        }
        due
    }

    /// True once, when the song or chapter has `left` seconds of listening, as long as the
    /// fade: the fade then ends with it.
    pub fn end_due(&mut self, left: f64) -> bool {
        let due = self.end && left <= SLEEP_FADE.as_secs_f64();
        if due {
            self.end = false;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sleep_timer_steps_through_its_choices_and_counts_down() {
        let mut timer = SleepTimer::default();
        timer.step(1);
        assert_eq!(timer.left(), Some(Sleep::End), "the end of the song first");
        assert_eq!(timer.remaining(), None, "an end has no time to count");
        timer.step(1);
        assert_eq!(timer.left(), Some(Sleep::Minutes(5)));
        assert!(timer.remaining().is_some_and(|d| d.as_secs() > 295));
        for _ in 0..6 {
            timer.step(1);
        }
        assert_eq!(timer.left(), Some(Sleep::Minutes(120)), "two hours last");
        timer.step(0);
        assert_eq!(timer.left(), None, "and round to off");
        timer.step(1);
        timer.step(1);
        assert!(!timer.due(), "not yet");
        timer.at = Some(Instant::now());
        assert!(timer.due(), "up");
        assert_eq!(timer.left(), None, "off once up");
        timer.again();
        assert_eq!(
            timer.left(),
            Some(Sleep::Minutes(5)),
            "and on again, as chosen"
        );
        assert_eq!(
            crate::text::sleeping(Some(Duration::from_secs(899))),
            "Sleep 14:59"
        );
    }
}
