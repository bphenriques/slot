//! The shelf: where each audiobook was left, its bookmarks, and the book being listened to.
//! Kept on the card by each book's folder or file, so a rescan, or its tags changing, keeps them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::library::Library;

pub const SHELF_FILE: &str = "Config/audiobooks.txt";
/// How often a book playing on is written down; a pause or a change of book writes at once.
const EVERY: Duration = Duration::from_secs(30);

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
    places: BTreeMap<PathBuf, Place>,
    bookmarks: BTreeMap<PathBuf, Vec<Bookmark>>,
    /// The chapters of the part each was left in, as Muse told them, for before it does again.
    chapters: BTreeMap<PathBuf, (usize, Vec<(f64, String)>)>,
    current: Option<PathBuf>,
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
            chapters: BTreeMap::new(),
            current: None,
            dirty: false,
            written: None,
        }
    }

    /// An empty shelf: for screens shown without a card.
    pub fn none() -> &'static Shelf {
        static NONE: Shelf = Shelf::new();
        &NONE
    }

    pub fn read(root: &Path) -> Shelf {
        let mut shelf = Shelf::new();
        let text = std::fs::read_to_string(root.join(SHELF_FILE)).unwrap_or_default();
        for line in text.lines() {
            shelf.parse(line);
        }
        shelf
    }

    /// Where `book` was left; its start when it was not begun.
    pub fn place(&self, lib: &Library, book: usize) -> Place {
        self.places
            .get(&lib.books[book].key)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_place(&mut self, lib: &Library, book: usize, place: Place) {
        let key = &lib.books[book].key;
        if self.places.get(key) != Some(&place) {
            self.places.insert(key.clone(), place);
            self.dirty = true;
        }
    }

    pub fn bookmarks(&self, lib: &Library, book: usize) -> &[Bookmark] {
        self.bookmarks
            .get(&lib.books[book].key)
            .map_or(&[], Vec::as_slice)
    }

    /// Adds a bookmark, kept in the order they come in the book.
    pub fn add_bookmark(&mut self, lib: &Library, book: usize, mark: Bookmark) {
        let marks = self
            .bookmarks
            .entry(lib.books[book].key.clone())
            .or_default();
        let at = marks.partition_point(|m| (m.part, m.pos) <= (mark.part, mark.pos));
        marks.insert(at, mark);
        self.dirty = true;
    }

    pub fn remove_bookmark(&mut self, lib: &Library, book: usize, i: usize) {
        if let Some(marks) = self.bookmarks.get_mut(&lib.books[book].key) {
            if i < marks.len() {
                marks.remove(i);
                self.dirty = true;
            }
        }
    }

    /// The chapters of `book`'s `part`, when they are known.
    pub fn chapters(&self, lib: &Library, book: usize, part: usize) -> Option<&[(f64, String)]> {
        let (at, chapters) = self.chapters.get(&lib.books[book].key)?;
        (*at == part).then_some(chapters.as_slice())
    }

    pub fn set_chapters(
        &mut self,
        lib: &Library,
        book: usize,
        part: usize,
        chapters: &[(f64, String)],
    ) {
        let key = &lib.books[book].key;
        if self
            .chapters
            .get(key)
            .is_some_and(|(at, c)| *at == part && c == chapters)
        {
            return;
        }
        self.chapters.insert(key.clone(), (part, chapters.to_vec()));
        self.dirty = true;
    }

    /// The book listened to, when the library still has it.
    pub fn current(&self, lib: &Library) -> Option<usize> {
        let key = self.current.as_ref()?;
        lib.books.iter().position(|b| &b.key == key)
    }

    pub fn set_current(&mut self, lib: &Library, book: Option<usize>) {
        let key = book.map(|b| lib.books[b].key.clone());
        if self.current != key {
            self.current = key;
            self.dirty = true;
        }
    }

    /// The shelf as its file has it, when it changed and is due a write: at once, or, while a
    /// book plays `on`, now and then.
    pub fn due(&mut self, on: bool) -> Option<String> {
        let due = !on || self.written.is_none_or(|at| at.elapsed() >= EVERY);
        if !self.dirty || !due {
            return None;
        }
        (self.dirty, self.written) = (false, Some(Instant::now()));
        Some(self.text())
    }

    /// One line a fact, the first field saying which.
    fn text(&self) -> String {
        let mut text = String::new();
        for (key, p) in &self.places {
            let finished = u8::from(p.finished);
            text += &format!(
                "place\t{}\t{}\t{:.1}\t{:.2}\t{finished}\t{}\n",
                key.display(),
                p.part,
                p.pos,
                p.speed,
                p.when
            );
        }
        for (key, marks) in &self.bookmarks {
            for m in marks {
                let chapter = m.chapter.replace(['\t', '\n'], " ");
                let key = key.display();
                text += &format!("bookmark\t{key}\t{}\t{:.1}\t{chapter}\n", m.part, m.pos);
            }
        }
        for (key, (part, chapters)) in &self.chapters {
            for (at, title) in chapters {
                let title = title.replace(['\t', '\n'], " ");
                let key = key.display();
                text += &format!("chapter\t{key}\t{part}\t{at:.1}\t{title}\n");
            }
        }
        if let Some(key) = &self.current {
            text += &format!("current\t{}\n", key.display());
        }
        text
    }

    fn parse(&mut self, line: &str) -> Option<()> {
        let mut f = line.split('\t');
        let kind = f.next()?;
        let key = PathBuf::from(f.next()?);
        match kind {
            "place" => {
                let place = Place {
                    part: f.next()?.parse().ok()?,
                    pos: f.next()?.parse().ok()?,
                    speed: f.next()?.parse().ok()?,
                    finished: f.next()? == "1",
                    when: f.next()?.parse().ok()?,
                };
                self.places.insert(key, place);
            }
            "bookmark" => {
                let mark = Bookmark {
                    part: f.next()?.parse().ok()?,
                    pos: f.next()?.parse().ok()?,
                    chapter: f.next().unwrap_or_default().into(),
                };
                self.bookmarks.entry(key).or_default().push(mark);
            }
            "chapter" => {
                let part = f.next()?.parse().ok()?;
                let at = f.next()?.parse().ok()?;
                let title = f.next().unwrap_or_default().into();
                let (was, chapters) = self.chapters.entry(key).or_insert((part, Vec::new()));
                if *was != part {
                    (*was, *chapters) = (part, Vec::new());
                }
                chapters.push((at, title));
            }
            "current" => self.current = Some(key),
            _ => {}
        }
        Some(())
    }
}

/// Seconds since 1970, for when a book was last listened to.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shelf_reads_back_as_written() {
        let mut shelf = Shelf::new();
        for line in [
            "place\tJust So\t3\t61.5\t1.25\t0\t1700000000",
            "bookmark\tJust So\t1\t20.0\tThe Whale",
            "chapter\tJust So\t3\t0.0\tThe Whale",
            "chapter\tJust So\t3\t61.5\tThe Camel",
            "current\tJust So",
            "an older line\tis skipped",
        ] {
            shelf.parse(line);
        }
        let text = shelf.text();
        let mut again = Shelf::new();
        text.lines().for_each(|l| {
            again.parse(l);
        });
        assert_eq!(again.text(), text);
        let place = again.places[Path::new("Just So")];
        assert_eq!((place.part, place.speed, place.when), (3, 1.25, 1700000000));
        assert_eq!(
            again.bookmarks[Path::new("Just So")][0].chapter,
            "The Whale"
        );
        assert_eq!(again.current.as_deref(), Some(Path::new("Just So")));
    }
}
