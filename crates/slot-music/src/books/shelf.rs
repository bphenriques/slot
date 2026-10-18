//! The shelf: where each audiobook was left, its bookmarks, and the book being listened to.
//! Kept on the card by each book's folder or file, so a rescan, or its tags changing, keeps them;
//! the file is the store's.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::library::{BookId, Library};

/// How often a book playing on is written down; a pause or a change of book writes at once.
const EVERY: Duration = Duration::from_secs(60);

/// Where a book was left.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Place {
    /// Its part, and the seconds into it.
    pub part: usize,
    pub pos: f64,
    pub speed: f32,
    pub finished: bool,
    /// When it was last listened to, in seconds since 1970; 0 for never.
    pub when: u64,
}

impl Place {
    /// The start of a book, at `speed`.
    pub fn start(speed: f32) -> Place {
        Place {
            part: 0,
            pos: 0.0,
            speed,
            finished: false,
            when: 0,
        }
    }
}

impl Default for Place {
    fn default() -> Self {
        Place::start(1.0)
    }
}

/// A moment marked to come back to, and the chapter it is in, as it was named then.
#[derive(Clone, PartialEq, Debug)]
pub struct Bookmark {
    pub part: usize,
    pub pos: f64,
    pub chapter: String,
}

/// Every book's place and bookmarks, by its key; and the book listened to, while it is.
pub struct Shelf {
    pub(crate) places: BTreeMap<PathBuf, Place>,
    pub(crate) bookmarks: BTreeMap<PathBuf, Vec<Bookmark>>,
    pub(crate) current: Option<PathBuf>,
    /// Changed since written, and when last written.
    dirty: bool,
    written: Option<Instant>,
}

impl Default for Shelf {
    fn default() -> Self {
        Shelf::new()
    }
}

impl Shelf {
    pub const fn new() -> Shelf {
        Shelf {
            places: BTreeMap::new(),
            bookmarks: BTreeMap::new(),
            current: None,
            dirty: false,
            written: None,
        }
    }

    /// An empty shelf: for screens shown without a card.
    #[cfg(test)]
    pub fn none() -> &'static Shelf {
        static NONE: Shelf = Shelf::new();
        &NONE
    }

    /// Where `book` was left; its start when it was not begun.
    pub fn place(&self, lib: &Library, book: BookId) -> Place {
        self.places
            .get(&lib.books[book].key)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_place(&mut self, lib: &Library, book: BookId, place: Place) {
        let key = &lib.books[book].key;
        if self.places.get(key) != Some(&place) {
            self.places.insert(key.clone(), place);
            self.dirty = true;
        }
    }

    pub fn bookmarks(&self, lib: &Library, book: BookId) -> &[Bookmark] {
        self.bookmarks
            .get(&lib.books[book].key)
            .map_or(&[], Vec::as_slice)
    }

    /// Adds a bookmark, kept in the order they come in the book.
    pub fn add_bookmark(&mut self, lib: &Library, book: BookId, mark: Bookmark) {
        let marks = self
            .bookmarks
            .entry(lib.books[book].key.clone())
            .or_default();
        let at = marks.partition_point(|m| (m.part, m.pos) <= (mark.part, mark.pos));
        marks.insert(at, mark);
        self.dirty = true;
    }

    pub fn remove_bookmark(&mut self, lib: &Library, book: BookId, i: usize) {
        if let Some(marks) = self.bookmarks.get_mut(&lib.books[book].key) {
            if i < marks.len() {
                marks.remove(i);
                self.dirty = true;
            }
        }
    }

    /// The book listened to, when the library still has it.
    pub fn current(&self, lib: &Library) -> Option<BookId> {
        let key = self.current.as_ref()?;
        lib.books.iter().position(|b| &b.key == key).map(BookId)
    }

    pub fn set_current(&mut self, lib: &Library, book: Option<BookId>) {
        let key = book.map(|b| lib.books[b].key.clone());
        if self.current != key {
            self.current = key;
            self.dirty = true;
        }
    }

    /// Whether the shelf changed and is due a write: at once, or, while a book plays `on`, now
    /// and then.
    pub fn due(&mut self, on: bool, now: Instant) -> bool {
        let due = !on
            || self
                .written
                .is_none_or(|at| now.saturating_duration_since(at) >= EVERY);
        if !self.dirty || !due {
            return false;
        }
        (self.dirty, self.written) = (false, Some(now));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_book_playing_is_written_down_each_minute_and_a_pause_at_once() {
        let t0 = Instant::now();
        let mut shelf = Shelf::new();
        shelf.dirty = true;
        assert!(shelf.due(true, t0), "the first time, at once");
        shelf.dirty = true;
        assert!(
            !shelf.due(true, t0 + EVERY / 2),
            "then not within the minute"
        );
        assert!(shelf.due(true, t0 + EVERY), "a minute on");
        shelf.dirty = true;
        assert!(shelf.due(false, t0 + EVERY), "paused, at once");
        assert!(
            !shelf.due(false, t0 + EVERY),
            "nothing changed, nothing to write"
        );
    }
}
