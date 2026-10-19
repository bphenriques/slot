//! Settings' Help: a few paragraphs on what the screens do not say, read as plain lyrics are,
//! the one in the middle moved by Up and Down.

use super::{Body, Screen, TextBody};
use crate::browse::{Browser, Key, Outcome};
use crate::{text, Music};

pub(crate) fn press(b: &mut Browser, key: Key) -> Outcome {
    let last = text::TIPS_TEXT.len() - 1;
    match key {
        Key::Up => b.set_at(b.at().saturating_sub(1)),
        Key::Down => b.set_at((b.at() + 1).min(last)),
        // As on the menus, the shoulders skip the song.
        Key::L1 => return Outcome::Skip(-1),
        Key::R1 => return Outcome::Skip(1),
        _ => {}
    }
    Outcome::None
}

pub(crate) fn screen(music: &Music) -> Screen {
    let body = TextBody {
        lines: text::TIPS_TEXT.map(String::from).to_vec(),
        lit: None,
        focus: music.browser.at(),
    };
    let legend = super::legend(&music.browser, &music.context());
    Screen::new(text::HELP, legend, Body::Text(body))
}
