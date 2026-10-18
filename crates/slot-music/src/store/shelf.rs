//! The books' shelf on the card: one line a fact, tab-separated, the first field saying which:
//! a book's place, a bookmark, and the book listened to. Older files' chapter lines
//! are skipped.

use std::path::PathBuf;

use super::{Card, Kept};
use crate::books::{Bookmark, Place, Shelf};

impl Kept for Shelf {
    const FILE: &'static str = "Config/audiobooks.txt";

    fn encode(&self, _: Card) -> String {
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
        if let Some(key) = &self.current {
            text += &format!("current\t{}\n", key.display());
        }
        text
    }

    /// Lines it cannot read, or of a kind it does not know, are skipped.
    fn decode(text: &str, _: Card) -> Shelf {
        let mut shelf = Shelf::new();
        for line in text.lines() {
            parse(&mut shelf, line);
        }
        shelf
    }
}

fn parse(shelf: &mut Shelf, line: &str) -> Option<()> {
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
            shelf.places.insert(key, place);
        }
        "bookmark" => {
            let mark = Bookmark {
                part: f.next()?.parse().ok()?,
                pos: f.next()?.parse().ok()?,
                chapter: f.next().unwrap_or_default().into(),
            };
            shelf.bookmarks.entry(key).or_default().push(mark);
        }
        "current" => shelf.current = Some(key),
        _ => {}
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::library::Library;

    #[test]
    fn the_shelf_reads_back_as_written() {
        let lib = Library::default();
        let card = Card {
            root: Path::new("/card"),
            lib: &lib,
        };
        let text = [
            "place\tJust So\t3\t61.5\t1.25\t0\t1700000000",
            "bookmark\tJust So\t1\t20.0\tThe Whale",
            "chapter\tJust So\t3\t0.0\tThe Whale",
            "chapter\tJust So\t3\t61.5\tThe Camel",
            "current\tJust So",
            "an older line\tis skipped",
        ]
        .join("\n");
        let shelf = Shelf::decode(&text, card);
        let again = Shelf::decode(&shelf.encode(card), card);
        assert_eq!(again.encode(card), shelf.encode(card));
        let place = again.places[Path::new("Just So")];
        assert_eq!((place.part, place.speed, place.when), (3, 1.25, 1700000000));
        assert_eq!(
            again.bookmarks[Path::new("Just So")][0].chapter,
            "The Whale"
        );
        assert_eq!(again.current.as_deref(), Some(Path::new("Just So")));
    }
}
