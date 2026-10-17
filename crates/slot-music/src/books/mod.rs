//! Audiobooks, played through the same player as the music, with the music's queue set aside
//! meanwhile and put back when a book ends. Each book's place, speed and bookmarks are kept on the shelf;
//! a book picks up a little back after a pause, by how long it was paused.

mod chapters;
mod shelf;

pub use shelf::{Bookmark, Place, Shelf, SHELF_FILE};

use crate::library::Library;
use crate::muse::{Command, Event, PlayState};
use crate::queue::Queue;
use crate::settings::cycle;
use crate::text::Notice;
use crate::Music;

/// The speeds X goes round, as players offer them.
const SPEEDS: [f32; 6] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// How far back a book picks up after `paused` seconds: nothing after a moment, the last
/// sentence after a while, more after hours.
fn rewind(paused: u64) -> f64 {
    match paused {
        0..5 => 0.0,
        5..300 => 5.0,
        300..3600 => 15.0,
        _ => 30.0,
    }
}

/// Seconds into `book`, `pos` into its `part`, by its parts' lengths.
pub fn into_book(lib: &Library, book: usize, part: usize, pos: f64) -> f64 {
    let parts = &lib.albums[lib.books[book].album].tracks;
    let before = parts[..part.min(parts.len())].iter();
    before.map(|&t| lib.tracks[t].secs).sum::<f64>() + pos
}

/// How far through `book`, 0 to 1, at `place`: by its parts' lengths, or by its parts alone
/// when a length is not known.
pub fn progress(lib: &Library, book: usize, place: &Place) -> f64 {
    let b = &lib.books[book];
    let parts = &lib.albums[b.album].tracks;
    let done = match b.secs > 0.0 {
        true => into_book(lib, book, place.part, place.pos) / b.secs,
        false => place.part as f64 / parts.len().max(1) as f64,
    };
    done.clamp(0.0, 1.0)
}

impl Music {
    /// The book playing, and which of its parts.
    pub(crate) fn book(&self) -> Option<(usize, usize)> {
        let t = self.player.current()?;
        let book = self.library.tracks[t].book?;
        let parts = &self.library.albums[self.library.books[book].album].tracks;
        Some((book, parts.iter().position(|&p| p == t).unwrap_or(0)))
    }

    /// Plays `book` where it was left, a little back, or from its start; at its own speed.
    pub(crate) fn play_book(&mut self, book: usize, over: bool) -> Option<Command> {
        self.note_book();
        let lib = &self.library;
        let place = match self.shelf.place(lib, book) {
            p if over || p.finished => Place::start(p.speed),
            p => p,
        };
        let at = place.pos - rewind(shelf::now().saturating_sub(place.when));
        self.open_book(book, place.part, at.max(0.0))
    }

    /// Plays `book` from `pos` into its `part`, at its speed, the music's queue set aside.
    fn open_book(&mut self, book: usize, part: usize, pos: f64) -> Option<Command> {
        if self.book().is_none() {
            self.aside = self.player.take();
        }
        let lib = &self.library;
        let parts = lib.albums[lib.books[book].album].tracks.len();
        let speed = self.shelf.place(lib, book).speed;
        self.shelf.set_current(lib, Some(book));
        self.back = None;
        self.player.set_speed(speed);
        self.play_part(book, part.min(parts.saturating_sub(1)), pos)
    }

    /// Music chosen while a book plays replaces the book, and the queue set aside.
    pub(crate) fn leave_book(&mut self) {
        if self.book().is_some() {
            self.note_book();
            self.shelf.set_current(&self.library, None);
            self.aside = None;
            self.back = None;
        }
    }

    /// The music's queue, to add to: set aside while a book plays.
    pub(crate) fn music_queue(&mut self) -> &mut Queue {
        match self.book() {
            Some(_) => &mut self.aside.get_or_insert_with(|| (Queue::default(), 0.0)).0,
            None => self.player.edit(),
        }
    }

    /// The music's queue, its place and the seconds into it, for saving: the queue file never
    /// holds a book.
    pub(crate) fn music_snapshot(&self) -> (&[usize], usize, f64) {
        match (self.book(), &self.aside) {
            (None, _) => self.player.snapshot(),
            (Some(_), Some((queue, pos))) => (queue.tracks(), queue.at(), *pos),
            (Some(_), None) => (&[], 0, 0.0),
        }
    }

    /// The book listened to when the card was last read, put back paused over the music.
    pub(crate) fn restore_book(&mut self) {
        let Some(book) = self.shelf.current(&self.library) else {
            return;
        };
        let place = self.shelf.place(&self.library, book);
        let parts = self.library.albums[self.library.books[book].album]
            .tracks
            .clone();
        let part = place.part.min(parts.len().saturating_sub(1));
        self.aside = self.player.take();
        self.player.set_speed(place.speed);
        let at = place.pos - rewind(shelf::now().saturating_sub(place.when));
        let stop = self.player.put(Queue::new(parts, part, None), at.max(0.0));
        self.send(stop);
        self.recall_chapters(book, part);
    }

    /// Before a paused book plays on: back a little, by how long it was paused.
    pub(crate) fn resume_book(&mut self) -> Option<Command> {
        let (book, _) = self.book()?;
        if self.player.state() != PlayState::Paused {
            return None;
        }
        let when = self.shelf.place(&self.library, book).when;
        self.player
            .rewind(rewind(shelf::now().saturating_sub(when)))
    }

    /// Notes where the book playing is, and when it last played; written down now and then
    /// while it plays, and at once otherwise.
    pub(crate) fn note_book(&mut self) {
        let playing = self.player.state() == PlayState::Playing;
        let open = self.player.state() != PlayState::Stopped;
        if let Some((book, part)) = self.book().filter(|_| open) {
            let was = self.shelf.place(&self.library, book);
            let place = Place {
                part,
                pos: self.player.time().0,
                speed: self.player.speed(),
                finished: false,
                when: if playing { shelf::now() } else { was.when },
            };
            self.shelf.set_place(&self.library, book, place);
            let chapters = self.player.chapter_list();
            if !chapters.is_empty() {
                self.shelf.set_chapters(&self.library, book, part, chapters);
            }
        }
        if let Some(text) = self.shelf.due(playing) {
            self.shelf_file.save(text);
        }
    }

    /// Whether `event` ends the last part of the book playing.
    pub(crate) fn ends_book(&self, event: &Event) -> bool {
        let Event::End(path) = event else {
            return false;
        };
        self.book().is_some_and(|(book, part)| {
            let parts = &self.library.albums[self.library.books[book].album].tracks;
            part + 1 == parts.len() && self.library.tracks[parts[part]].path == *path
        })
    }

    /// The book playing came to its end: finished, from its start next time; the music's
    /// queue comes back, paused.
    pub(crate) fn finish_book(&mut self) {
        let Some((book, _)) = self.book() else {
            return;
        };
        let place = Place {
            finished: true,
            when: shelf::now(),
            ..Place::start(self.player.speed())
        };
        self.shelf.set_place(&self.library, book, place);
        self.shelf.set_current(&self.library, None);
        if let Some((queue, pos)) = self.aside.take() {
            let stop = self.player.put(queue, pos);
            self.send(stop);
        }
    }

    /// Marks `book` finished, or not, from its start either way.
    pub(crate) fn toggle_finished(&mut self, book: usize) {
        let was = self.shelf.place(&self.library, book);
        let place = Place {
            finished: !was.finished,
            ..Place::start(was.speed)
        };
        self.shelf.set_place(&self.library, book, place);
    }

    /// The speed `by` steps to, as a setting steps: Left and Right stop at the ends, X and A go
    /// round.
    pub(crate) fn step_speed(&mut self, by: i32) -> Option<Command> {
        let speed = cycle(&SPEEDS, self.player.speed(), by);
        self.tell(Notice::Speed((speed * 100.0).round() as u16));
        self.player.set_speed(speed)
    }

    /// Goes to chapter `i`: the file's own, or else a part.
    pub(crate) fn go_chapter(&mut self, i: usize) -> Option<Command> {
        match self.player.chapters() {
            0 => self.player.jump(&self.library, i),
            _ => self.player.go_chapter(i),
        }
    }

    /// Marks where the book playing is, named by its chapter.
    pub(crate) fn add_bookmark(&mut self) -> Option<Command> {
        let (book, part) = self.book()?;
        let chapter = match self.player.chapter() {
            Some((_, title, ..)) => title.to_string(),
            None => self.library.tracks[self.player.current()?].title.clone(),
        };
        let pos = self.player.smooth_pos();
        let mark = Bookmark { part, pos, chapter };
        self.shelf.add_bookmark(&self.library, book, mark);
        self.tell(Notice::Bookmarked)
    }

    /// Plays `book` from its bookmark `i`: within the part playing, a seek.
    pub(crate) fn go_bookmark(&mut self, book: usize, i: usize) -> Option<Command> {
        let mark = self.shelf.bookmarks(&self.library, book).get(i)?.clone();
        match self.book() {
            Some((b, part)) if b == book && part == mark.part => self.player.seek_to(mark.pos),
            _ => {
                self.note_book();
                self.open_book(book, mark.part, mark.pos)
            }
        }
    }

    pub(crate) fn remove_bookmark(&mut self, book: usize, i: usize) {
        self.shelf.remove_bookmark(&self.library, book, i);
    }

    /// Before a skip, a chapter or a bookmark moves the book: where it was, to go back to.
    pub(crate) fn remember_place(&mut self) {
        if let Some((book, part)) = self.book() {
            self.back = Some((book, part, self.player.smooth_pos()));
        }
    }

    /// Back to where the book was before it last moved, which can then be gone back to.
    pub(crate) fn go_back(&mut self) -> Option<Command> {
        let (book, part, pos) = self.back.take()?;
        let (now, at) = self.book()?;
        if now != book {
            return None;
        }
        self.back = Some((book, at, self.player.smooth_pos()));
        match part == at {
            true => self.player.seek_to(pos),
            false => self.play_part(book, part, pos),
        }
    }

    /// Plays `book` from `pos` into its `part`, with the chapters it had.
    fn play_part(&mut self, book: usize, part: usize, pos: f64) -> Option<Command> {
        let parts = self.library.albums[self.library.books[book].album]
            .tracks
            .clone();
        let play = self
            .player
            .play_at(&self.library, Queue::new(parts, part, None), pos);
        self.recall_chapters(book, part);
        play
    }

    /// The chapters of `book`'s `part` until Muse tells them: an MP4's own, or else as Muse
    /// last told them.
    fn recall_chapters(&mut self, book: usize, part: usize) {
        let lib = &self.library;
        let path = &lib.tracks[lib.albums[lib.books[book].album].tracks[part]].path;
        let chapters = match chapters::read(path) {
            read if !read.is_empty() => read,
            _ => self
                .shelf
                .chapters(lib, book, part)
                .unwrap_or_default()
                .to_vec(),
        };
        if !chapters.is_empty() {
            self.player.set_chapters(chapters);
        }
    }

    /// Seconds of listening left in the book's chapter playing, for the sleep timer.
    pub(crate) fn chapter_left(&self) -> Option<f64> {
        self.book()?;
        let (pos, len) = (self.player.smooth_pos(), self.player.time().1);
        let end = self.player.chapter().map_or(len, |c| c.3);
        (len > 0.0).then(|| (end - pos) / f64::from(self.player.speed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_book_picks_up_further_back_the_longer_it_was_paused() {
        let back: Vec<f64> = [2, 60, 1200, 86400].map(rewind).into();
        assert_eq!(back, [0.0, 5.0, 15.0, 30.0]);
    }
}
