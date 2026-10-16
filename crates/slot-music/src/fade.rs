//! The volume fading in on resume, and the sleep timer that fades the music out.

use std::time::{Duration, Instant};

use crate::settings::cycle;

/// How long a resume fades in, and the sleep timer's last, slower fade.
pub const FADE: Duration = Duration::from_millis(300);
pub const SLEEP_FADE: Duration = Duration::from_secs(8);

/// A volume ramp from `from` to `to`, 0 silent to 1 full.
#[derive(Copy, Clone)]
pub struct Fade {
    from: f32,
    to: f32,
    since: Instant,
    over: Duration,
}

impl Fade {
    pub fn new(from: f32, to: f32, over: Duration) -> Fade {
        let since = Instant::now();
        Fade {
            from,
            to,
            since,
            over,
        }
    }

    /// A fade down to silence, which pauses at its end.
    pub fn is_out(&self) -> bool {
        self.to == 0.0
    }

    pub fn done(&self) -> bool {
        self.since.elapsed() >= self.over
    }

    pub fn level(&self) -> f32 {
        let u = self.since.elapsed().as_secs_f32() / self.over.as_secs_f32();
        self.from + (self.to - self.from) * u.min(1.0)
    }
}

/// What the sleep timer waits for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sleep {
    /// Minutes, rounded up.
    Minutes(u32),
    /// The end of the audiobook's chapter playing.
    Chapter,
}

/// Pauses after a chosen number of minutes, or at the end of a book's chapter.
#[derive(Default)]
pub struct SleepTimer {
    at: Option<Instant>,
    chapter: bool,
}

impl SleepTimer {
    const CHOICES: [Option<Sleep>; 6] = [
        None,
        Some(Sleep::Chapter),
        Some(Sleep::Minutes(15)),
        Some(Sleep::Minutes(30)),
        Some(Sleep::Minutes(45)),
        Some(Sleep::Minutes(60)),
    ];

    /// What it waits for, with the minutes left; `None` when off.
    pub fn left(&self) -> Option<Sleep> {
        if self.chapter {
            return Some(Sleep::Chapter);
        }
        let left = self.at?.saturating_duration_since(Instant::now());
        Some(Sleep::Minutes(left.as_secs().div_ceil(60) as u32))
    }

    /// Steps from the choice at or above the minutes left, as other settings step; the end of
    /// the chapter is a choice only in an audiobook.
    pub fn step(&mut self, by: i32, book: bool) {
        let choices: Vec<Option<Sleep>> = SleepTimer::CHOICES
            .into_iter()
            .filter(|&c| book || c != Some(Sleep::Chapter))
            .collect();
        let shown = match self.left() {
            Some(Sleep::Minutes(left)) => choices.iter().copied().find(|c| match c {
                Some(Sleep::Minutes(m)) => *m >= left,
                _ => false,
            }),
            left => Some(left),
        };
        let to = cycle(&choices, shown.flatten(), by);
        self.chapter = to == Some(Sleep::Chapter);
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

    /// True once, when the chapter has `left` seconds of listening, as long as the fade: the
    /// fade then ends with it.
    pub fn chapter_due(&mut self, left: f64) -> bool {
        let due = self.chapter && left <= SLEEP_FADE.as_secs_f64();
        if due {
            self.chapter = false;
        }
        due
    }
}
