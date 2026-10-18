//! Playing audiobooks: one put on over the music, whose queue waits aside until the book ends
//! or music plays; its place kept on the shelf as it plays, and its chapters, bookmarks and
//! speed.

use super::worker::Job;
use super::{Effect, Music};
use crate::books::shelf::{Bookmark, Place, Shelf};
use crate::books::{rewind, SPEEDS};
use crate::library::{BookId, TrackId};
use crate::playback::queue::Queue;
use crate::settings::cycle;
use crate::store::Kept;
use crate::text::Notice;
use slot_player::{Command, Event, PlayState};

/// Where each audiobook was left; the music's queue set aside while one plays, and where in
/// it; and where the book was before a skip, a chapter or a bookmark moved it.
pub(crate) struct Books {
    pub(crate) shelf: Shelf,
    pub(crate) aside: Option<(Queue, f64)>,
    pub(crate) back: Option<Spot>,
}

/// A place in a book: its part, and seconds into it.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Spot {
    pub book: BookId,
    pub part: usize,
    pub pos: f64,
}

impl Music {
    /// The book playing, and which of its parts.
    pub(crate) fn book(&self) -> Option<(BookId, usize)> {
        let t = self.player.current()?;
        let book = self.library.tracks[t].book?;
        let parts = &self.library.albums[self.library.books[book].album].tracks;
        Some((book, parts.iter().position(|&p| p == t).unwrap_or(0)))
    }

    /// Plays `book` where it was left, a little back, or from its start; at its own speed.
    pub(crate) fn play_book(&mut self, book: BookId, over: bool) -> Option<Command> {
        self.note_book();
        let lib = &self.library;
        let place = match self.books.shelf.place(lib, book) {
            p if over || p.finished => Place::start(p.speed),
            p => p,
        };
        let at = place.pos - rewind(self.now.wall.saturating_sub(place.when));
        self.open_book(book, place.part, at.max(0.0))
    }

    /// Plays `book` from `pos` into its `part`, at its speed, the music's queue set aside.
    fn open_book(&mut self, book: BookId, part: usize, pos: f64) -> Option<Command> {
        if self.book().is_none() {
            self.books.aside = self.player.take();
        }
        let lib = &self.library;
        let parts = lib.albums[lib.books[book].album].tracks.len();
        let speed = self.books.shelf.place(lib, book).speed;
        self.books.shelf.set_current(lib, Some(book));
        self.books.back = None;
        self.player.set_speed(speed);
        self.play_part(book, part.min(parts.saturating_sub(1)), pos)
    }

    /// Music chosen while a book plays replaces the book, and the queue set aside.
    pub(crate) fn leave_book(&mut self) {
        if self.book().is_some() {
            self.note_book();
            self.books.shelf.set_current(&self.library, None);
            self.books.aside = None;
            self.books.back = None;
        }
    }

    /// The music's queue, to add to: set aside while a book plays.
    pub(crate) fn music_queue(&mut self) -> &mut Queue {
        match self.book() {
            Some(_) => {
                &mut self
                    .books
                    .aside
                    .get_or_insert_with(|| (Queue::default(), 0.0))
                    .0
            }
            None => self.player.edit(),
        }
    }

    /// The music's queue, its place and the seconds into it, for saving: the queue file never
    /// holds a book.
    pub(crate) fn music_snapshot(&self) -> (&[TrackId], usize, f64) {
        match (self.book(), &self.books.aside) {
            (None, _) => self.player.snapshot(),
            (Some(_), Some((queue, pos))) => (queue.tracks(), queue.at(), *pos),
            (Some(_), None) => (&[], 0, 0.0),
        }
    }

    /// The book listened to when the card was last read, put back over the music: paused, or
    /// playing on when it is the file still `playing`, after a rescan. True when it plays on.
    pub(crate) fn restore_book(&mut self, playing: Option<&std::path::Path>) -> bool {
        let Some(book) = self.books.shelf.current(&self.library) else {
            return false;
        };
        let place = self.books.shelf.place(&self.library, book);
        let parts = self.library.albums[self.library.books[book].album]
            .tracks
            .clone();
        let part = place.part.min(parts.len().saturating_sub(1));
        let plays_on = playing == Some(self.library.tracks[parts[part]].path.as_path());
        self.books.aside = self.player.take();
        self.player.set_speed(place.speed);
        match plays_on {
            true => self.player.restore(parts, part, place.pos, true),
            false => {
                let at = place.pos - rewind(self.now.wall.saturating_sub(place.when));
                let stop = self.player.put(Queue::new(parts, part, None), at.max(0.0));
                self.send(stop);
            }
        }
        plays_on
    }

    /// Before a paused book plays on: back a little, by how long it was paused.
    pub(crate) fn resume_book(&mut self) -> Option<Command> {
        let (book, _) = self.book()?;
        if self.player.state() != PlayState::Paused {
            return None;
        }
        let when = self.books.shelf.place(&self.library, book).when;
        self.player
            .rewind(rewind(self.now.wall.saturating_sub(when)))
    }

    /// Notes where the book playing is, and when it last played; written down now and then
    /// while it plays, and at once otherwise.
    pub(crate) fn note_book(&mut self) {
        let playing = self.player.state() == PlayState::Playing;
        let open = self.player.state() != PlayState::Stopped;
        if let Some((book, part)) = self.book().filter(|_| open) {
            let was = self.books.shelf.place(&self.library, book);
            let place = Place {
                part,
                pos: self.player.time().0,
                speed: self.player.speed(),
                finished: false,
                when: if playing { self.now.wall } else { was.when },
            };
            self.books.shelf.set_place(&self.library, book, place);
        }
        if self.books.shelf.due(playing, self.now.at) {
            let text = self.books.shelf.encode(self.card());
            self.out.push(Effect::Write(Shelf::FILE.into(), text));
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
            when: self.now.wall,
            ..Place::start(self.player.speed())
        };
        self.books.shelf.set_place(&self.library, book, place);
        self.books.shelf.set_current(&self.library, None);
        if let Some((queue, pos)) = self.books.aside.take() {
            let stop = self.player.put(queue, pos);
            self.send(stop);
        }
    }

    /// Marks `book` finished, or not, from its start either way.
    pub(crate) fn toggle_finished(&mut self, book: BookId) {
        let was = self.books.shelf.place(&self.library, book);
        let place = Place {
            finished: !was.finished,
            ..Place::start(was.speed)
        };
        self.books.shelf.set_place(&self.library, book, place);
    }

    /// The speed `by` steps to, as a setting steps: Left and Right stop at the ends, X and A go
    /// round from 0.75, and from 0.5 on to 1.
    pub(crate) fn step_speed(&mut self, by: i32) -> Option<Command> {
        let order = if by == 0 { &SPEEDS[1..] } else { &SPEEDS[..] };
        let speed = cycle(order, self.player.speed(), by);
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
            Some(c) if !c.chapter.title.is_empty() => c.chapter.title.clone(),
            _ => self.library.tracks[self.player.current()?].title.clone(),
        };
        let pos = self.player.time().0;
        let mark = Bookmark { part, pos, chapter };
        self.books.shelf.add_bookmark(&self.library, book, mark);
        self.tell(Notice::Bookmarked)
    }

    /// Plays `book` from its bookmark `i`: within the part playing, a seek.
    pub(crate) fn go_bookmark(&mut self, book: BookId, i: usize) -> Option<Command> {
        let mark = self
            .books
            .shelf
            .bookmarks(&self.library, book)
            .get(i)?
            .clone();
        match self.book() {
            Some((b, part)) if b == book && part == mark.part => self.player.seek_to(mark.pos),
            _ => {
                self.note_book();
                self.open_book(book, mark.part, mark.pos)
            }
        }
    }

    pub(crate) fn remove_bookmark(&mut self, book: BookId, i: usize) {
        self.books.shelf.remove_bookmark(&self.library, book, i);
    }

    /// Before a skip, a chapter or a bookmark moves the book: where it was, to go back to.
    pub(crate) fn remember_place(&mut self) {
        if let Some((book, part)) = self.book() {
            let pos = self.player.time().0;
            self.books.back = Some(Spot { book, part, pos });
        }
    }

    /// Back to where the book was before it last moved, which can then be gone back to.
    pub(crate) fn go_back(&mut self) -> Option<Command> {
        let Spot { book, part, pos } = self.books.back.take()?;
        let (now, at) = self.book()?;
        if now != book {
            return None;
        }
        let here = self.player.time().0;
        self.books.back = Some(Spot {
            book,
            part: at,
            pos: here,
        });
        match part == at {
            true => self.player.seek_to(pos),
            false => self.play_part(book, part, pos),
        }
    }

    /// Plays `book` from `pos` into its `part`.
    fn play_part(&mut self, book: BookId, part: usize, pos: f64) -> Option<Command> {
        let parts = self.library.albums[self.library.books[book].album]
            .tracks
            .clone();
        self.player
            .play_at(&self.library, Queue::new(parts, part, None), pos)
    }

    /// Asks for the chapters of the book's part playing, and the next part's, read ahead so
    /// the bar does not jump as one runs into the other: once a part, however it was reached,
    /// read from its file by the worker, an MP4's own or an MP3's.
    pub(crate) fn read_chapters(&mut self) {
        let Some(track) = self.player.current().filter(|_| self.book().is_some()) else {
            return;
        };
        // Read before it played, they are the player's as it starts.
        if self.player.chapters_of() != Some(track) {
            if let Some(chapters) = self.chapters.of(track) {
                self.player.set_chapters(track, chapters.clone());
            }
        }
        let wanted: Vec<_> = [Some(track), self.player.up_next()]
            .into_iter()
            .flatten()
            .collect();
        for part in self.chapters.want(&wanted) {
            let path = self.library.tracks[part].path.clone();
            self.work(Job::Chapters { track: part, path });
        }
    }

    /// Seconds of listening left in the song playing, or the book's chapter, for the sleep
    /// timer.
    pub(crate) fn end_left(&self) -> Option<f64> {
        let (pos, len) = (self.player.time().0, self.player.time().1);
        let end = self.player.chapter().map_or(len, |c| c.end);
        let speed = self.book().map_or(1.0, |_| f64::from(self.player.speed()));
        (len > 0.0).then(|| (end - pos) / speed)
    }
}
