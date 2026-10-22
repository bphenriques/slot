//! What each key does on a screen, as data, as niri declares its binds: how it acts while down
//! (once, again and again, tapped or held), and what its hint says. A screen's legend is chosen
//! once (`screens::legend`); its keys act, and its hints read, from the one table, so a hint
//! never says what its key does not do.

use crate::browse::keys::Mode;
use crate::browse::Key;
use crate::text::Word;

use super::{Legend, MainHold, Target};

/// A key on a screen: how it acts while down, and its hint, if it has one: what a tap does, and
/// what a hold does, either of which it may not.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Bind {
    pub key: Key,
    pub mode: Mode,
    pub hint: Option<(Option<Word>, Option<Word>)>,
}

/// Hinted, acting tapped.
fn tap(key: Key, word: Word) -> Bind {
    Bind {
        key,
        mode: Mode::Once,
        hint: Some((Some(word), None)),
    }
}

/// Hinted, a tap one thing and a hold another: "Play (Next)".
fn hold(key: Key, tapped: Word, held: Word) -> Bind {
    Bind {
        key,
        mode: Mode::Hold,
        hint: Some((Some(tapped), Some(held))),
    }
}

/// Hinted, acting only held, as a deletion does, its tap doing nothing: "(Delete)".
fn held(key: Key, word: Word) -> Bind {
    Bind {
        key,
        mode: Mode::Hold,
        hint: Some((None, Some(word))),
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
        // A book plays on, X lists its chapters, Y its bookmarks, and START has its menu.
        Legend::List(Target::Book) => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                tap(Key::A, Word::Play),
                tap(Key::X, Word::Chapters),
                tap(Key::Y, Word::Bookmarks),
            ];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        Legend::List(Target::Author) => {
            let mut binds = vec![tap(Key::B, Word::Back), tap(Key::A, Word::Open)];
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        // A also puts the row's songs next when held, X adds them to the queue, Y puts a song in
        // Favorites or takes it out, and START has a song's menu, as a book's list has.
        Legend::List(target) => {
            // Play All plays, held it shuffles; a row of the list plays or opens, held it is next.
            let a = match target {
                Target::All => hold(Key::A, Word::Play, Word::Shuffle),
                Target::Song => hold(Key::A, Word::Play, Word::Next),
                _ => hold(Key::A, Word::Open, Word::Next),
            };
            let mut binds = vec![tap(Key::B, Word::Back), a, tap(Key::X, Word::AddToQueue)];
            if target == Target::Song {
                binds.push(tap(Key::Y, Word::Favorite));
            }
            binds.extend(repeat(&MOVES));
            binds.extend(repeat(&PAGES));
            binds
        }
        // A held puts the track next, but the one playing is already there. X held goes on
        // taking tracks out; Y held lifts one to carry. START saves or clears it all.
        Legend::Queue { restart } => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                if restart {
                    tap(Key::A, Word::Restart)
                } else {
                    hold(Key::A, Word::Play, Word::Next)
                },
                acting(tap(Key::X, Word::Remove), Mode::Repeat),
                hold(Key::Y, Word::Favorite, Word::Move),
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
            // SELECT opens the queue, as it does from anywhere: Help says so, not the hints.
            binds.push(tap(Key::Y, Word::Favorite));
            binds.extend(BACK_KEYS.map(|k| quiet(k, Mode::HoldRepeat)));
            binds
        }
        // Scrolled to a line of timed lyrics, A plays from it.
        Legend::Lyrics { playing, line } => {
            let a = if line { Word::PlayLine } else { play(playing) };
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
        // A uses the preset; X held deletes one's own, as the queue's X takes a song out.
        Legend::Equalizer { own } => {
            let mut binds = vec![tap(Key::B, Word::Back), tap(Key::A, Word::Choose)];
            binds.extend(own.then(|| held(Key::X, Word::Delete)));
            binds.extend(repeat(&MOVES));
            binds
        }
        // Left and Right move a band, again and again held, as the arrows say; Y keeps them.
        Legend::Tune => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                acting(tap(Key::Left, Word::Change), Mode::Repeat),
                tap(Key::Y, Word::Save),
                quiet(Key::Right, Mode::Repeat),
            ];
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
        // Update Library reads what changed; held, it reads everything again.
        Legend::Update => {
            let mut binds = vec![
                tap(Key::B, Word::Back),
                hold(Key::A, Word::Update, Word::Rebuild),
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
    // Everywhere: B held goes home and START held pauses or plays; SELECT acts as it is let go,
    // as a shoulder pressed with it skips instead, taking its tap. Unhinted unless a screen
    // says otherwise.
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

/// With the screen off, the keys found blind: the shoulders, which seek held, and the triggers,
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
    use crate::text;

    /// Every screen's table, and the screen off's.
    fn tables() -> Vec<(String, Vec<Bind>)> {
        let mut all: Vec<(String, Vec<Bind>)> = Legend::all()
            .into_iter()
            .map(|legend| (format!("{legend:?}"), binds(legend)))
            .collect();
        all.push(("the screen off".into(), dark(false)));
        all.push(("the screen off on a book".into(), dark(true)));
        all
    }

    #[test]
    fn no_screen_hints_more_than_four_keys_so_the_hints_fit_in_one_row() {
        for (name, binds) in tables() {
            let hinted = binds.iter().filter(|b| b.hint.is_some()).count();
            assert!(hinted <= 4, "{name}: {hinted} hints");
        }
    }

    #[test]
    fn every_screen_b_goes_back_from_hints_back_but_the_playing_screens() {
        for legend in Legend::all() {
            let playing = matches!(
                legend,
                Legend::NowPlaying { .. } | Legend::Lyrics { .. } | Legend::Book { .. }
            );
            let back = binds(legend)
                .iter()
                .any(|b| b.key == Key::B && b.hint == Some((Some(Word::Back), None)));
            let carry = legend == Legend::Carry;
            assert_eq!(back, !playing && !carry, "{legend:?}");
        }
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
        // Held anywhere, as Controls says: B goes home, START pauses or plays.
        let everywhere = [Key::B, Key::Start];
        // With the screen off nothing is seen, so nothing is hinted; Help says it.
        let lit = tables()
            .into_iter()
            .filter(|(name, _)| !name.starts_with("the screen off"));
        for (name, binds) in lit {
            for bind in binds {
                let holds = matches!(bind.mode, Mode::Hold | Mode::HoldRepeat);
                let said = bind.hint.and_then(|(_, held)| held);
                assert!(
                    said.is_none() || holds,
                    "{name}: {:?} says a hold it has not",
                    bind.key
                );
                let known = everywhere.contains(&bind.key);
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
