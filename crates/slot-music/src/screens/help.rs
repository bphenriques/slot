//! Help: a few tips, then the keys the hints leave out, read as plain lyrics are, the line in
//! the middle moved by Up and Down. Shown on the first start, and from Settings; B closes it.

use super::{Body, Screen, TextBody};
use crate::browse::{Browser, Key, Outcome};
use crate::{text, Music};

pub(crate) fn press(b: &mut Browser, key: Key) -> Outcome {
    let last = text::help().len() - 1;
    match key {
        Key::Up => b.set_at(b.at().saturating_sub(1)),
        Key::Down => b.set_at((b.at() + 1).min(last)),
        _ => {}
    }
    Outcome::None
}

pub(crate) fn screen(music: &Music) -> Screen {
    let body = TextBody {
        lines: text::help(),
        lit: None,
        focus: music.browser.at(),
    };
    let legend = super::legend(&music.browser, &music.context());
    Screen::new(text::HELP, legend, Body::Text(body))
}
