//! What each key does on a screen, as data, as niri declares its binds: how it acts while down
//! (once, again and again, tapped or held), and what its hint says. A screen's legend is chosen
//! once (`screens::legend`); its keys act, and its hints read, from the one table, so a hint
//! never says what its key does not do.

use crate::browse::keys::Mode;
use crate::browse::Key;
use crate::text::Word;

use super::{Legend, MainHold, Target};

/// A key on a screen: how it acts while down, and its hint, if it has one: what a tap does, and
/// what a hold does.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Bind {
    pub key: Key,
    pub mode: Mode,
    pub hint: Option<(Word, Option<Word>)>,
}

/// Hinted, acting tapped.
fn tap(key: Key, word: Word) -> Bind {
    Bind {
        key,
        mode: Mode::Once,
        hint: Some((word, None)),
    }
}

/// Hinted, a tap one thing and a hold another: "Play (Next)".
fn hold(key: Key, tapped: Word, held: Word) -> Bind {
    Bind {
        key,
        mode: Mode::Hold,
        hint: Some((tapped, Some(held))),
    }
}

/// Unhinted, as `mode` has it.
fn quiet(key: Key, mode: Mode) -> Bind {
    Bind {
        key,
        mode,
        hint: None,
    }
}

/// `bind`, acting as `mode`.
fn acting(bind: Bind, mode: Mode) -> Bind {
    Bind { mode, ..bind }
}

/// Up and Down, and in a list Left and Right and the shoulders: they move, again and again.
const MOVES: [Key; 2] = [Key::Up, Key::Down];
const PAGES: [Key; 4] = [Key::Left, Key::Right, Key::L1, Key::R1];
/// The back keys on Now Playing and the lyrics: tapped, they skip or jump; held, they seek.
const BACK_KEYS: [Key; 6] = [Key::Left, Key::Right, Key::L1, Key::R1, Key::L2, Key::R2];

/// The keys of `legend`'s screen, hinted ones in the order their hints show.
pub(crate) fn binds(legend: Legend) -> Vec<Bind> {
    let pick = |on: bool, yes, no| if on { yes } else { no };
    let play = |playing| pick(playing, Word::Pause, Word::Play);
    let repeat = |keys: &[Key]| {
        keys.iter()
            .map(|&k| quiet(k, Mode::Repeat))
            .collect::<Vec<_>>()
    };
    let mut binds: Vec<Bind> = match legend {
        // SELECT goes to what plays, so it shows only when something does. A held on a list
        // shuffles it all; on Audiobooks, plays on the latest book.
        Legend::Main {
            playing,
            hold: held,
        } => {
            let open = match held {
                Some(MainHold::Shuffle) => hold(Key::A, Word::Open, Word::Shuffle),
                Some(MainHold::Resume) => hold(Key::A, Word::Open, Word::Resume),
                None => tap(Key::A, Word::Open),
            };
            let mut binds = vec![tap(Key::B, Word::Back), open];
            binds.extend(playing.then(|| tap(Key::Select, Word::Playing)));
            binds.extend(repeat(&MOVES));
            binds
        }
        // A book plays on, Y lists its bookmarks and START has its menu.
        Legend::List(Target::Book) => {
            let mut binds = vec![
                tap(Key::A, Word::Play),
                tap(Key::Y, Word::Bookmarks),
                tap(Key::Start, Word::Menu),
            ];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        Legend::List(Target::Author) => {
            let mut binds = vec![tap(Key::A, Word::Open)];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        // A also puts the row's songs next when held, X adds them to the queue, and Y puts a
        // song in Favorites or takes it out. A song's START menu goes unsaid: it only goes to its
        // album or artist, or its info.
        Legend::List(target) => {
            let open = match target {
                Target::Song | Target::All => Word::Play,
                _ => Word::Open,
            };
            let mut binds = vec![
                hold(Key::A, open, Word::Next),
                tap(Key::X, Word::AddToQueue),
            ];
            binds.extend((target == Target::Song).then(|| tap(Key::Y, Word::Favorite)));
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        // A held puts the track next, but the one playing is already there. X held goes on
        // taking tracks out; Y held lifts one to carry. START saves or clears it all.
        Legend::Queue { restart } => {
            let mut binds = vec![
                match restart {
                    true => tap(Key::A, Word::Restart),
                    false => hold(Key::A, Word::Play, Word::Next),
                },
                acting(tap(Key::X, Word::Remove), Mode::Repeat),
                hold(Key::Y, Word::Favorite, Word::Move),
                tap(Key::Start, Word::Menu),
            ];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        Legend::Carry => {
            let mut binds = vec![
                tap(Key::B, Word::Cancel),
                tap(Key::Y, Word::Drop),
                tap(Key::R1, Word::PlayNext),
            ];
            binds.extend(repeat(&MOVES));
            binds
        }
        // X shows only where there are lyrics, as Apple Music dims its button.
        Legend::NowPlaying { playing, lyrics } => {
            let mut binds = vec![tap(Key::A, play(playing))];
            binds.extend(lyrics.then(|| tap(Key::X, Word::Lyrics)));
            binds.extend([tap(Key::Y, Word::Favorite), tap(Key::Select, Word::Queue)]);
            binds.extend(BACK_KEYS.map(|k| quiet(k, Mode::HoldRepeat)));
            binds
        }
        // Scrolled to a line of timed lyrics, A plays from it.
        Legend::Lyrics { playing, line } => {
            let a = match line {
                true => Word::PlayLine,
                false => play(playing),
            };
            let mut binds = vec![tap(Key::A, a), tap(Key::X, Word::Close)];
            binds.extend(repeat(&MOVES));
            binds.extend(BACK_KEYS.map(|k| quiet(k, Mode::HoldRepeat)));
            binds
        }
        // Left and Right jump, as the arrows say; X steps the speed, which START's menu sets
        // too; Y marks the moment, or held lists the marks.
        Legend::Book { playing, jump } => {
            let mut binds = vec![
                tap(Key::A, play(playing)),
                acting(tap(Key::Left, Word::Jump(jump)), Mode::HoldRepeat),
                tap(Key::X, Word::Speed),
                hold(Key::Y, Word::Bookmark, Word::List),
            ];
            binds.extend(BACK_KEYS[1..].iter().map(|&k| quiet(k, Mode::HoldRepeat)));
            binds
        }
        Legend::Chapters => {
            let mut binds = vec![tap(Key::B, Word::Back), tap(Key::A, Word::Play)];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        Legend::Bookmarks => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                tap(Key::A, Word::Play),
                tap(Key::X, Word::Remove),
            ];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        Legend::Settings => {
            let mut binds = vec![tap(Key::B, Word::Back), tap(Key::Left, Word::Change)];
            binds.extend(repeat(&MOVES));
            binds
        }
        Legend::Menu => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                tap(Key::A, Word::Open),
                tap(Key::Left, Word::Change),
            ];
            binds.extend(repeat(&MOVES));
            binds
        }
        // Rebuild acts only when A is held, as it takes long; the hint says so in its words.
        Legend::Hold => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                acting(tap(Key::A, Word::HoldToRebuild), Mode::Hold),
            ];
            binds.extend(repeat(&MOVES));
            binds
        }
        Legend::Back => {
            let mut binds = vec![tap(Key::B, Word::Back)];
            binds.extend(repeat(&MOVES));
            binds
        }
    };
    // Everywhere: B held goes home, START held pauses or plays, and SELECT acts as it is let go,
    // so a shoulder with it held skips instead. Unhinted unless a screen says otherwise.
    for (key, mode) in [
        (Key::B, Mode::Hold),
        (Key::Start, Mode::Hold),
        (Key::Select, Mode::Release),
    ] {
        match binds.iter_mut().find(|b| b.key == key) {
            Some(bind) => bind.mode = mode,
            None => binds.push(quiet(key, mode)),
        }
    }
    binds
}

/// With the lid closed, the keys it leaves: the shoulders, which seek held, and the triggers,
/// which play or pause as they are let go, so either may yet be one of the two that lock; in a
/// `book`, held, they mark the moment. The rest do nothing, and nothing is hinted.
pub(crate) fn dark(book: bool) -> Vec<Bind> {
    let trigger = if book { Mode::Hold } else { Mode::Release };
    vec![
        quiet(Key::L1, Mode::HoldRepeat),
        quiet(Key::R1, Mode::HoldRepeat),
        quiet(Key::L2, trigger),
        quiet(Key::R2, trigger),
    ]
}

/// How `key` acts on a screen with `binds`: as its bind says; a key with none, once.
pub(crate) fn mode(binds: &[Bind], key: Key) -> Mode {
    binds
        .iter()
        .find(|b| b.key == key)
        .map_or(Mode::Once, |b| b.mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{self, Word};

    /// Every screen's table, and the lid's.
    fn tables() -> Vec<(String, Vec<Bind>)> {
        let mut all: Vec<(String, Vec<Bind>)> = Legend::all()
            .into_iter()
            .map(|legend| (format!("{legend:?}"), binds(legend)))
            .collect();
        all.push(("the lid closed".into(), dark(false)));
        all.push(("the lid closed on a book".into(), dark(true)));
        all
    }

    #[test]
    fn a_key_has_one_meaning_on_a_screen() {
        for (name, binds) in tables() {
            for (i, bind) in binds.iter().enumerate() {
                let again = binds[i + 1..].iter().any(|b| b.key == bind.key);
                assert!(!again, "{name}: {:?} twice", bind.key);
            }
        }
    }

    #[test]
    fn a_hint_says_a_hold_only_of_a_key_that_holds_and_a_key_that_holds_says_so() {
        // Held anywhere, as CONTROLS.md has it: B goes home, START pauses or plays.
        let everywhere = [Key::B, Key::Start];
        // With the lid closed nothing is seen, so nothing is hinted; CONTROLS.md says it.
        let lit = tables()
            .into_iter()
            .filter(|(name, _)| !name.starts_with("the lid"));
        for (name, binds) in lit {
            for bind in binds {
                let holds = matches!(bind.mode, Mode::Hold | Mode::HoldRepeat);
                let said = bind.hint.and_then(|(_, held)| held);
                assert!(
                    said.is_none() || holds,
                    "{name}: {:?} says a hold it has not",
                    bind.key
                );
                // Rebuild says it in so many words: "Hold to Rebuild".
                let in_words = bind.hint.is_some_and(|(tap, _)| tap == Word::HoldToRebuild);
                let known = everywhere.contains(&bind.key) || in_words;
                assert!(
                    bind.mode != Mode::Hold || said.is_some() || known,
                    "{name}: {:?} holds, and its hint does not say what",
                    bind.key
                );
            }
        }
    }

    #[test]
    fn every_hint_has_a_cap_to_show_it_on() {
        for (name, binds) in tables() {
            let hinted = binds.iter().filter(|b| b.hint.is_some());
            for bind in hinted {
                assert!(
                    text::cap(bind.key).is_some(),
                    "{name}: {:?} has no cap",
                    bind.key
                );
            }
        }
    }
}
