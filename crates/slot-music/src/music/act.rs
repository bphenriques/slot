//! What a key does: the screen shown says, as an `Outcome`, and this carries it out on the
//! state, asking the world outside only through effects.

use std::time::Duration;

use super::{seed, Effect, Music};
use crate::browse::keys::Press;
use crate::browse::{Key, Outcome, Shuffle, View};

use crate::playback::queue::Queue;
use crate::screens;
use crate::settings::Setting;
use crate::store::playlists;
use crate::store::settings::changes;
use crate::text::{self, Notice};
use slot_player::Command;

impl Music {
    /// A key tapped or held: what it does on the screen shown. True when it leaves the music
    /// screen.
    pub(crate) fn act(&mut self, press: Press<Key>) -> bool {
        // The pocket combo: tapped, play or pause, locked or not; held, lock or unlock.
        match press {
            Press::Tap(Key::Pocket) => {
                let toggle = self.toggle();
                self.send(toggle);
                return false;
            }
            Press::Held(Key::Pocket) => {
                let each = Duration::from_millis(u64::from(self.settings.buzz));
                self.pocket
                    .toggle(self.settings.pocket_buzz.then_some(each));
                self.tell(Notice::Locked(self.pocket.locked));
                return false;
            }
            _ => {}
        }
        // The browser moves while the rest is read: taken out for the while.
        let mut browser = std::mem::take(&mut self.browser);
        let outcome = {
            let cx = self.context();
            match (self.dark, press) {
                (true, press) => {
                    let jump = cx.book.map(|_| cx.settings.jump as i32);
                    screens::dark::act(press, jump)
                }
                (false, Press::Tap(key)) => browser.press(&cx, key),
                (false, Press::Held(key)) => browser.held(&cx, key),
            }
        };
        self.browser = browser;
        if matches!(
            outcome,
            Outcome::Skip(_) | Outcome::Chapter(_) | Outcome::GoBookmark { .. }
        ) {
            self.remember_place();
        }
        let command = match outcome {
            Outcome::None => None,
            Outcome::Leave => return true,
            Outcome::Rescan(full) => {
                self.rescan(full);
                return false;
            }
            Outcome::Toggle => {
                // Away from Now Playing, the corner says what START held did.
                let was = self.player.queue().playing;
                if self.browser.view() != View::NowPlaying && self.player.current().is_some() {
                    self.tell(if was { Notice::Paused } else { Notice::Playing });
                }
                self.toggle()
            }
            Outcome::Tell(notice) => self.tell(notice),
            Outcome::TestBuzz => {
                let each = Duration::from_millis(u64::from(self.settings.buzz));
                self.pocket.buzz(1, each);
                None
            }
            Outcome::PlayBook { book, over } => self.play_book(book, over),
            Outcome::Finish(book) => {
                self.toggle_finished(book);
                None
            }
            Outcome::Speed(by) => self.step_speed(by),
            Outcome::Chapter(i) => self.go_chapter(i),
            Outcome::Bookmark => self.add_bookmark(),
            Outcome::GoBookmark { book, mark } => self.go_bookmark(book, mark),
            Outcome::RemoveBookmark { book, mark } => {
                self.remove_bookmark(book, mark);
                None
            }
            Outcome::Back => self.go_back(),
            Outcome::Clear => {
                self.player.edit().clear();
                None
            }
            Outcome::SaveQueue => {
                let tracks = self.player.queue().tracks.to_vec();
                if let Some(p) = self.library.add_playlist(text::SAVED_QUEUE, tracks) {
                    self.write_playlist(p);
                }
                self.tell(Notice::Saved)
            }
            // The heart on Now Playing says it.
            Outcome::Favorite(t) => {
                if let Some(p) = self.library.toggle_favorite(t) {
                    self.write_playlist(p);
                }
                None
            }
            Outcome::Remove { at, n } => {
                for _ in 0..n {
                    self.player.edit().remove(at);
                }
                None
            }
            Outcome::Move(from, to) => {
                self.player.edit().move_to(from, to);
                None
            }
            Outcome::Letter(letter) => {
                self.letter = Some((letter, std::time::Instant::now()));
                None
            }
            Outcome::ShuffleQueue => {
                self.player.edit().toggle_shuffle(seed());
                None
            }
            Outcome::Change(Setting::SleepTimer, by) => {
                self.transport.sleep.step(by);
                None
            }
            Outcome::Change(setting, by) => {
                let was = self.settings;
                self.settings.change(setting, by);
                self.player.set_repeat(self.settings.repeat);
                for (key, value) in changes(&self.settings, &was) {
                    self.out.push(Effect::Setting(key, value));
                }
                match setting {
                    Setting::PerfLog => {
                        self.out.push(Effect::PerfLog(self.settings.perf));
                        None
                    }
                    Setting::Mono => Some(Command::Mono(self.settings.mono)),
                    _ => None,
                }
            }
            Outcome::Seek(by) => self.player.seek(by),
            Outcome::FromLine(i) => {
                let at = self.lyrics_now().and_then(|l| l.lines.get(i)?.at);
                at.and_then(|at| self.player.seek_to(at))
            }
            Outcome::Skip(by) => self.player.skip(&self.library, by),
            Outcome::Jump(to) => self.player.jump(&self.library, to),
            Outcome::PlayNext(tracks) => {
                self.music_queue().play_next(&tracks);
                self.tell(Notice::Next(tracks.len()))
            }
            Outcome::Enqueue(tracks) => {
                self.music_queue().enqueue(&tracks);
                self.tell(Notice::Added(tracks.len()))
            }
            Outcome::Play {
                tracks,
                start,
                shuffle,
            } => {
                let queue = match shuffle {
                    Shuffle::Off => Queue::new(tracks, start, None),
                    Shuffle::Tracks => Queue::new(tracks, start, Some(seed())),
                    Shuffle::Groups(sizes) => Queue::groups(tracks, &sizes, seed()),
                };
                self.leave_book();
                self.player.play(&self.library, queue)
            }
        };
        self.send(command);
        self.keep_queue();
        self.note_book();
        false
    }

    /// Writes playlist `p` back, whole.
    fn write_playlist(&mut self, p: usize) {
        let list = &self.library.playlists[p];
        let text = playlists::encode(list, &self.library);
        self.out.push(Effect::Write(list.path.clone(), text));
    }
}
