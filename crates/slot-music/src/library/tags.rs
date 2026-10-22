//! A song's tags and front picture, read with lofty. Nothing else knows the tag formats.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::Tag;

/// What a song's tags say; empty or 0 where they say nothing.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub disc: u32,
    pub track: u32,
    pub year: u32,
    /// Its genre as tagged, perhaps several: "Rock; Pop".
    pub genre: String,
    /// How long it plays, in seconds: read for audiobooks, whose progress needs it.
    pub secs: u32,
}

/// The tags of the song at `path`, skipping its pictures, and how long it plays, for the queue's
/// time left and how far into a book; none when it has none or cannot be read.
pub fn read(path: &Path) -> Tags {
    let tagged = Probe::open(path).and_then(|p| {
        p.options(
            ParseOptions::new()
                .read_cover_art(false)
                .read_properties(true),
        )
        .read()
    });
    let Ok(tagged) = tagged else {
        return Tags::default();
    };
    let secs = tagged.properties().duration().as_secs() as u32;
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Tags {
            secs,
            ..Tags::default()
        };
    };
    let text = |s: Option<std::borrow::Cow<str>>| s.map(|s| clean(&s)).unwrap_or_default();
    Tags {
        title: text(tag.title()),
        artist: text(tag.artist()),
        album_artist: tag
            .get_string(ItemKey::AlbumArtist)
            .map(clean)
            .unwrap_or_default(),
        album: text(tag.album()),
        disc: tag.disk().unwrap_or(0),
        track: tag.track().unwrap_or(0),
        year: tag.date().map_or(0, |d| u32::from(d.year)),
        genre: text(tag.genre()),
        secs,
    }
}

/// The song's front cover, or else its first picture.
pub fn picture(path: &Path) -> Option<Vec<u8>> {
    let tagged = Probe::open(path)
        .and_then(|p| p.options(ParseOptions::new().read_properties(false)).read())
        .ok()?;
    let pictures = tagged.tags().iter().flat_map(Tag::pictures);
    let front = pictures
        .clone()
        .find(|p| p.pic_type() == PictureType::CoverFront);
    front
        .or_else(|| pictures.clone().next())
        .map(|p| p.data().to_vec())
}

/// One line, trimmed: tabs and line breaks would break the library's cache.
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ").trim().to_string()
}
