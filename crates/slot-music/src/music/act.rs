//! What a key does: the screen shown says, as an `Outcome`, and this carries it out on the
//! state, asking the world outside only through effects.

use super::{Effect, Music};
use crate::browse::keys::Press;
use crate::browse::{Key, Outcome, Shuffle, View};
use crate::library::sort::initial;

use crate::playback::queue::Queue;
use crate::screens;
use crate::settings::Setting;
use crate::store::playlists;
use crate::store::settings::changes;
use crate::text::{self, Notice};

impl Music {
    /// A key tapped or held: what it does on the screen shown. True when it leaves the music
    /// screen.
    pub(crate) fn act(&mut self, press: Press<Key>) -> bool {
        // The pocket combo held locks or unlocks; tapped, it does nothing, as a key alone plays or
        // pauses.
        match press {
            Press::Tap(Key::Pocket) => return false,
            Press::Held(Key::Pocket) => {
                self.buttons.pocket.toggle(self.now.at);
                self.tell(Notice::Locked(self.buttons.pocket.locked));
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
        self.follow_letter(press);
        if matches!(
            outcome,
            Outcome::Skip(_) | Outcome::BookChapter { .. } | Outcome::GoBookmark { .. }
        ) {
            self.remember_place();
        }
        let command = match outcome {
            Outcome::None => None,
            Outcome::Leave => return true,
            // Back to the Music menu, where the reading shows, its bar filling.
            Outcome::Rescan(full) => {
                self.browser.home();
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
            Outcome::PlayBook { book, over } => self.play_book(book, over),
            Outcome::Finish(book) => {
                self.toggle_finished(book);
                None
            }
            Outcome::Speed(by) => self.step_speed(by),
            Outcome::BookChapter {
                book,
                part,
                chapter,
            } => self.go_book_chapter(book, part, chapter),
            // Marked with the screen off, a buzz says so, as nothing is seen.
            Outcome::Bookmark => {
                if self.dark {
                    self.buttons.pocket.tick(self.now.at);
                }
                self.add_bookmark()
            }
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
                self.tell(Notice::Saved);
                None
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
                self.flash.letter = Some((letter, self.now.at));
                None
            }
            Outcome::ShuffleQueue => {
                self.player.edit().toggle_shuffle(self.now.seed);
                None
            }
            Outcome::Change(Setting::SleepTimer, by) => {
                self.transport.sleep.step(by, self.now.at);
                None
            }
            Outcome::Equalizer(i) => {
                self.choose_preset(i);
                None
            }
            Outcome::NewPreset => {
                self.new_preset();
                None
            }
            Outcome::Tune { band, by } => {
                self.tune(band, by);
                None
            }
            Outcome::SavePreset => {
                self.save_preset();
                None
            }
            Outcome::DeletePreset(i) => {
                self.delete_preset(i);
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
                    #[cfg(feature = "perf")]
                    Setting::PerfLog => {
                        self.out.push(Effect::PerfLog(self.settings.perf));
                        None
                    }
                    Setting::Log => {
                        self.out.push(Effect::Log(self.settings.log));
                        None
                    }
                    _ => None,
                }
            }
            Outcome::Seek(by) => {
                // Unseen, a jump that lands in another chapter buzzes once, to say so.
                let was = self.player.chapter().map(|c| c.index);
                let seek = self.player.seek(by);
                if self.dark && was != self.player.chapter().map(|c| c.index) {
                    self.buttons.pocket.tick(self.now.at);
                }
                seek
            }
            Outcome::FromLine(i) => {
                let at = self.lyrics_now().and_then(|l| l.lines.get(i)?.at);
                at.and_then(|at| self.player.seek_to(at))
            }
            Outcome::Skip(by) => {
                // Away from Now Playing and the lyrics, the corner says it went on or back.
                let seen = matches!(self.browser.view(), View::NowPlaying | View::Lyrics);
                if !seen && !self.dark && self.player.current().is_some() {
                    self.tell(Notice::Skipped { on: by > 0 });
                }
                self.player.skip(&self.library, by)
            }
            Outcome::Jump(to) => self.player.jump(&self.library, to),
            Outcome::PlayNext(tracks) => {
                self.music_queue().play_next(&tracks);
                self.tell(Notice::Next(tracks.len()));
                None
            }
            Outcome::Enqueue(tracks) => {
                self.music_queue().enqueue(&tracks);
                self.tell(Notice::Added(tracks.len()));
                None
            }
            Outcome::Play {
                tracks,
                start,
                shuffle,
            } => {
                let queue = match shuffle {
                    Shuffle::Off => Queue::new(tracks, start, None),
                    Shuffle::Tracks => Queue::new(tracks, start, Some(self.now.seed)),
                    Shuffle::Groups(sizes) => Queue::groups(tracks, &sizes, self.now.seed),
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

impl Music {
    /// Moving fast through a list by name shows the letter it is at, as phones do: held, or a
    /// page at a time; once shown, the letter follows each move while it lasts.
    fn follow_letter(&mut self, press: Press<Key>) {
        let shown = self
            .flash
            .letter
            .is_some_and(|(_, at)| self.now.at.saturating_duration_since(at) < screens::LETTER);
        let fast = match press {
            Press::Held(Key::Up | Key::Down | Key::Left | Key::Right) => true,
            Press::Tap(Key::Left | Key::Right) => true,
            Press::Tap(Key::Up | Key::Down) => shown,
            _ => false,
        };
        if !fast || !self.browser.view().by_name() || self.dark {
            return;
        }
        let row = self.browser.row(&self.context());
        if let Some(name) = row.and_then(|row| crate::browse::rows::name(&self.library, row)) {
            self.flash.letter = Some((initial(name), self.now.at));
        }
    }
}
