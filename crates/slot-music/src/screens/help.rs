//! Help: a few tips, then the keys the hints leave out, a list to read rather than pick from,
//! set left as any list. Shown on the first start, and from Settings; B closes it.

use super::{list, Body, Screen};
use crate::Music;

pub(crate) fn screen(music: &Music) -> Screen {
    let mut screen = list::rows_screen(music);
    screen.corner.clear();
    if let Body::List(list) = &mut screen.body {
        list.readonly = true;
    }
    screen
}
