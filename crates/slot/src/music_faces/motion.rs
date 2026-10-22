//! The music screen's animations, apart from what is drawn: a new screen slides in, the
//! highlight glides, rows slide to new places as the list changes, and a new album picture fades
//! in.

use slot_input::Millis;
use slot_music::{Body, RowText, Screen};
use slot_music_ui::{
    marquee_start, music_arrived, music_eased, MusicMotion, MusicTween, MUSIC_ART_MS,
    MUSIC_GLIDE_MS, MUSIC_SLIDE_MS,
};

/// Rows moved further than this came from elsewhere, as a new page, and just arrive.
const MOVED_MAX: usize = 2;

#[derive(Default)]
pub(super) struct Motion {
    /// The screen shown, by title and depth, when it came in and from which side.
    shown: Option<(String, usize)>,
    entered: Millis,
    side: f32,
    /// The highlight, and the rows it glides over.
    bar: MusicTween,
    /// The text view's line in the middle, as it moves to another.
    focus: MusicTween,
    rows: Vec<RowText>,
    /// How many rows each row moved by when the rows last changed in place, and when: they
    /// slide from where they were.
    moved: Vec<f32>,
    moved_at: Millis,
    /// How long they slide for: shorter when changes come quicker than a slide.
    slide: Millis,
    /// The highlight rides with its row as it slides: the row was carried.
    rides: bool,
    art: Millis,
}

impl Motion {
    /// The music screen is hidden: it comes in afresh when it opens again.
    pub fn hide(&mut self) {
        self.shown = None;
    }

    /// How long the screen shown has been up, for a title too long for the header to scroll:
    /// from the marquee's beat, so it steps with the rest.
    pub fn up_for(&self, now: Millis) -> Millis {
        now.saturating_sub(marquee_start(self.entered))
    }

    /// A screen is shown. A new one slides in: deeper from the right, back from the left.
    pub fn show(&mut self, screen: &Screen, now: Millis) {
        // The lyrics are one screen whatever song they are of, titled by it: a song skipped to
        // changes them in place rather than bringing the screen in again.
        let title = match screen.body {
            Body::Text(_) => String::new(),
            _ => screen.title.clone(),
        };
        let key = (title, screen.depth);
        if self.shown.as_ref() == Some(&key) {
            return;
        }
        self.side = match self.shown.replace(key) {
            Some((_, was)) if was < screen.depth => 1.0,
            Some((_, was)) if was > screen.depth => -1.0,
            _ => 0.0,
        };
        self.entered = now;
    }

    /// Where the highlight is drawn, in rows: it glides within a page. When the rows change it
    /// holds still for them to slide under it, unless its own row moved, as a carried track
    /// does: then it rides along, so the row stays inside it.
    pub fn bar(&mut self, rows: &[RowText], selected: usize, now: Millis) -> f32 {
        let to = selected as f32;
        if self.rows != rows {
            let was = self.rows.get(self.bar.to as usize);
            self.rides = was.is_some_and(|was| rows.get(selected) == Some(was));
            // On the same screen, rows follow their change; a new one just arrives.
            if self.entered == now {
                self.moved = Vec::new()
            } else {
                self.follow(rows, now)
            }
            self.rows = rows.to_vec();
            self.bar = MusicTween::still(to, now);
        } else if to != self.bar.to {
            self.rides = false;
            self.bar.toward(to, now, MUSIC_GLIDE_MS);
        }
        let ride = if self.rides {
            self.offsets(now).get(selected).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        self.bar.at(now, MUSIC_GLIDE_MS) + ride
    }

    /// Where each row was before `rows` replaced the last ones: a row taken out leaves the
    /// rows under it to slide up, a moved one and those it passed slide to their places. Rows
    /// from far off, as on a new page, just arrive. A row still sliding goes on from where it
    /// is drawn, so a quick run of moves never jumps; and a run quicker than a slide slides as
    /// quickly, so rows keep up with a held key rather than trail further and further behind.
    fn follow(&mut self, rows: &[RowText], now: Millis) {
        let sliding = self.offsets(now);
        self.slide = now.saturating_sub(self.moved_at).clamp(1, MUSIC_SLIDE_MS);
        let mut used = vec![false; self.rows.len()];
        self.moved = (0..rows.len())
            .map(|i| {
                let was = (0..self.rows.len()).find(|&j| !used[j] && self.rows[j] == rows[i]);
                match was {
                    Some(j) if j.abs_diff(i) <= MOVED_MAX => {
                        used[j] = true;
                        j as f32 - i as f32 + sliding.get(j).copied().unwrap_or(0.0)
                    }
                    _ => 0.0,
                }
            })
            .collect();
        self.moved_at = now;
    }

    /// How far each row is from its place, in rows, as it slides there.
    pub fn offsets(&self, now: Millis) -> Vec<f32> {
        let u = self.eased(self.moved_at, now, self.slide);
        if u < 1.0 {
            self.moved.iter().map(|d| d * (1.0 - u)).collect()
        } else {
            Vec::new()
        }
    }

    /// Where the text view's middle is, in lines: it moves smoothly to `to`; on a new screen, or
    /// `new` text, as a song skipped to's lyrics, it starts there.
    pub fn focus(&mut self, to: f32, new: bool, now: Millis) -> f32 {
        if new || self.entered == now {
            self.focus = MusicTween::still(to, now)
        } else {
            self.focus.toward(to, now, MUSIC_SLIDE_MS)
        }
        self.focus.at(now, MUSIC_SLIDE_MS)
    }

    /// A new album picture is up: to `fade` in, or there at once.
    pub fn new_art(&mut self, now: Millis, fade: bool) {
        self.art = if fade {
            now
        } else {
            now.saturating_sub(MUSIC_ART_MS)
        };
    }

    /// Whether the screen shown is still coming in `now`.
    pub fn entering(&self, now: Millis) -> bool {
        self.arrived(now) < 1.0
    }

    /// Whether anything is still on its way `now`: a screen sliding in, the highlight gliding, rows
    /// sliding, the text view moving or a picture fading in. Moving, it is drawn again next frame.
    pub fn moving(&self, now: Millis) -> bool {
        let going = |since, over| self.eased(since, now, over) < 1.0;
        let gliding = |t: &MusicTween, over| t.from != t.to && going(t.since, over);
        self.entering(now)
            || gliding(&self.bar, MUSIC_GLIDE_MS)
            || gliding(&self.focus, MUSIC_SLIDE_MS)
            || (self.moved.iter().any(|&d| d != 0.0) && going(self.moved_at, self.slide))
            || going(self.art, MUSIC_ART_MS)
    }

    pub fn at(&self, now: Millis) -> MusicMotion {
        MusicMotion {
            enter: self.arrived(now),
            side: self.side,
            art: self.eased(self.art, now, MUSIC_ART_MS),
        }
    }

    /// How far the screen shown has come in `now`, 0 to 1.
    fn arrived(&self, now: Millis) -> f32 {
        music_arrived(now.saturating_sub(self.entered), MUSIC_SLIDE_MS)
    }

    /// How far along an ease begun at `since`, 0 to 1.
    fn eased(&self, since: Millis, now: Millis, over: Millis) -> f32 {
        music_eased(now.saturating_sub(since), over)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(names: &[&str]) -> Vec<RowText> {
        names.iter().map(|&n| RowText::plain(n)).collect()
    }

    /// A screen titled `title` at `depth`: the lyrics, or a list.
    fn screen(title: &str, depth: usize, lyrics: bool) -> Screen {
        let body = if lyrics {
            Body::Text(slot_music::TextBody {
                lines: Vec::new(),
                lit: None,
                focus: 0,
                progress: None,
                time: None,
            })
        } else {
            Body::Message(String::new())
        };
        Screen {
            title: title.into(),
            corner: String::new(),
            legend: slot_music::Legend::Back,
            body,
            depth,
            letter: None,
            hold: None,
        }
    }

    #[test]
    fn the_lyrics_of_a_song_skipped_to_change_in_place_and_start_at_their_line() {
        let mut m = Motion::default();
        let far = 100_000;
        m.show(&screen("One · Abba", 2, true), far);
        assert!(m.entering(far), "the lyrics come in");
        let later = far + 1000;
        m.focus(12.0, false, far);
        m.show(&screen("Two · Abba", 2, true), later);
        assert!(!m.entering(later), "the next song's: in place");
        assert_eq!(
            m.focus(0.0, true, later),
            0.0,
            "at their line, not gliding there"
        );
        m.show(&screen("Albums", 2, false), later + 10);
        assert!(m.entering(later + 10), "another screen still comes in");
    }

    #[test]
    fn it_is_moving_until_each_motion_has_arrived_and_at_rest_after() {
        let mut m = Motion::default();
        let far = 100_000;
        m.bar(&rows(&["a", "b", "c"]), 0, far);
        assert!(!m.moving(far), "at rest");
        m.bar(&rows(&["a", "b", "c"]), 2, far + 10);
        assert!(m.moving(far + 10), "the highlight glides");
        assert!(!m.moving(far + 10 + MUSIC_GLIDE_MS), "and arrives");
        m.bar(&rows(&["a", "c"]), 1, far + 1000);
        assert!(m.moving(far + 1000), "a row taken out, the rest slide up");
        assert!(!m.moving(far + 1000 + MUSIC_SLIDE_MS));
        m.new_art(far + 2000, true);
        assert!(
            m.moving(far + 2000 + MUSIC_ART_MS / 2),
            "a picture fades in"
        );
        assert!(!m.moving(far + 2000 + MUSIC_ART_MS));
        // One there at once does not fade, nor keep the frames coming.
        m.new_art(far + 3000, false);
        assert_eq!(m.at(far + 3000).art, 1.0);
        assert!(!m.moving(far + 3000));
    }

    #[test]
    fn the_highlight_rides_with_a_carried_row_and_holds_still_for_a_removed_one() {
        let mut m = Motion::default();
        m.bar(&rows(&["a", "b", "c"]), 1, 10);
        // b carried down: the highlight starts where b was and slides with it.
        assert_eq!(m.bar(&rows(&["a", "c", "b"]), 2, 20), 1.0);
        let done = 20 + MUSIC_SLIDE_MS;
        assert_eq!(m.bar(&rows(&["a", "c", "b"]), 2, done), 2.0);
        // b taken out: c slides up under a highlight that stays.
        m.bar(&rows(&["a", "b", "c"]), 1, done + 10);
        let later = done + 10 + MUSIC_SLIDE_MS;
        assert_eq!(m.bar(&rows(&["a", "c"]), 1, later), 1.0);
        let offsets = m.offsets(later);
        assert_eq!(offsets[1], 1.0, "c starts a row down");
    }

    #[test]
    fn a_row_moved_again_mid_slide_goes_on_from_where_it_is_drawn() {
        let mut m = Motion::default();
        m.bar(&rows(&["a", "b", "c", "d"]), 0, 10);
        // a carried down, then again before it lands.
        let start = 10 + MUSIC_SLIDE_MS;
        m.bar(&rows(&["b", "a", "c", "d"]), 1, start);
        let half = start + MUSIC_SLIDE_MS / 2;
        let drawn = 1.0 + m.offsets(half)[1];
        m.bar(&rows(&["b", "c", "a", "d"]), 2, half);
        assert_eq!(2.0 + m.offsets(half)[2], drawn, "no jump");
    }

    #[test]
    fn rows_keep_up_with_a_key_held_to_take_them_out() {
        let mut m = Motion::default();
        let names: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        let mut shown: Vec<&str> = names.iter().map(String::as_str).collect();
        m.bar(&rows(&shown), 0, 10);
        // One taken out every 50 ms: the first slide is cut short, then each ends before the
        // next, so the rows never trail more than the row just taken out.
        for step in 1..10 {
            shown.remove(0);
            let now = 10 + MUSIC_SLIDE_MS + step * 50;
            m.bar(&rows(&shown), 0, now);
            let most = if step <= 2 { 2.0 } else { 1.0 };
            assert!(m.offsets(now)[0] <= most, "trailing at step {step}");
        }
    }
}
