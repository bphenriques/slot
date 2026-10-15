use slot_store::initial;

use crate::library::Library;
use crate::settings::Settings;

/// Rows on a menu page (the music main screen, settings): the quick menu's.
pub const MENU_ROWS: usize = 6;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum View {
    Main,
    Queue,
    Settings,
    Artists,
    Artist(usize),
    Albums,
    Album(usize),
    Tracks,
    NowPlaying,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Row {
    NowPlaying,
    Artists,
    Albums,
    Tracks,
    Settings,
    Rescan,
    /// Settings.
    ScreenOff,
    SkipWith,
    ListSize,
    PlayAll,
    Shuffle,
    Artist(usize),
    Album(usize),
    Track(usize),
    /// A place in the queue.
    Queued(usize),
    ClearQueue,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    L1,
    R1,
    A,
    B,
    X,
    /// Y: play next in lists; on Now Playing, the next repeat mode.
    Y,
    Start,
    /// SELECT on its own: to Now Playing and back.
    Select,
    /// SELECT with L1 or R1: the previous or next track, from any music screen.
    SelectL1,
    SelectR1,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    None,
    Leave,
    Rescan,
    /// Replace the queue with `tracks`, playing from `start`.
    Play {
        tracks: Vec<usize>,
        start: usize,
        shuffle: Shuffle,
    },
    /// Play from this place in the queue.
    Jump(usize),
    /// Add to the end of the queue.
    Enqueue(Vec<usize>),
    /// Put right after what plays.
    PlayNext(Vec<usize>),
    Toggle,
    Seek(i32),
    Skip(i32),
    /// The repeat mode, one step on or back.
    Repeat(i32),
    /// Shuffle the queue around what plays, or put it back in order.
    ShuffleQueue,
    /// Change a setting: Left (-1) and Right (1) stop at the ends, A (0) wraps.
    Change(Row, i32),
    /// The cursor jumped to this letter.
    Letter(char),
    /// Empty what comes after the track playing.
    ClearUpNext,
    /// Take this place out of the queue.
    Remove(usize),
    /// Move the track at this place up (-1) or down (1).
    Move(usize, i32),
}

/// How Play orders tracks: as listed, shuffled, or as shuffled groups `sizes` long.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shuffle {
    Off,
    Tracks,
    Groups(Vec<usize>),
}

/// The queue as the browser sees it: its tracks, and the place of the one playing.
#[derive(Copy, Clone, Default)]
pub struct Queue<'a> {
    pub tracks: &'a [usize],
    pub at: usize,
    /// Whether it plays now, rather than paused or waiting.
    pub playing: bool,
}

/// A row: its name, dim context before (a track number) or after (the artist), an icon, and
/// its state.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RowText {
    pub icon: Option<RowIcon>,
    pub before: String,
    pub text: String,
    pub after: String,
    pub playing: bool,
    /// Set dim: played already, in the queue, or nothing to open.
    pub dim: bool,
    /// Acts on the whole list, like Play All.
    pub action: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RowIcon {
    Play,
}

impl RowText {
    pub fn plain(text: impl Into<String>) -> RowText {
        RowText {
            text: text.into(),
            ..RowText::default()
        }
    }
}

/// A row compares by its name.
impl PartialEq<&str> for RowText {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

/// A list screen: its title and the page the cursor is on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Page {
    pub title: String,
    pub rows: Vec<RowText>,
    /// A setting's value, or a count, beside its row.
    pub values: Vec<Option<String>>,
    pub selected: usize,
    pub page: usize,
    pub pages: usize,
    /// The highlighted item's place from 1, and how many items, not counting action rows.
    pub item: Option<usize>,
    pub items: usize,
    /// Rows this page holds.
    pub per_page: usize,
    /// Action rows at the top of this page.
    pub actions: usize,
}

struct Level {
    view: View,
    at: usize,
}

pub struct Browser {
    stack: Vec<Level>,
}

impl Default for Browser {
    fn default() -> Self {
        Browser {
            stack: vec![Level {
                view: View::Main,
                at: 0,
            }],
        }
    }
}

impl Browser {
    pub fn view(&self) -> View {
        self.top().view
    }

    pub fn press(&mut self, lib: &Library, queue: Queue, settings: &Settings, key: Key) -> Outcome {
        match key {
            Key::Start => return Outcome::Toggle,
            Key::SelectL1 => return Outcome::Skip(-1),
            Key::SelectR1 => return Outcome::Skip(1),
            Key::Select => {
                self.visit(View::NowPlaying);
                return Outcome::None;
            }
            _ => {}
        }
        if key == Key::B {
            return match self.stack.len() {
                1 => Outcome::Leave,
                _ => {
                    self.stack.pop();
                    Outcome::None
                }
            };
        }
        if self.view() == View::NowPlaying {
            let (back, on) = match settings.skip_with_dpad {
                true => ((Key::Left, Key::Right), (Key::L1, Key::R1)),
                false => ((Key::L1, Key::R1), (Key::Left, Key::Right)),
            };
            return match key {
                Key::A => Outcome::Toggle,
                Key::X => Outcome::ShuffleQueue,
                Key::Y => Outcome::Repeat(1),
                // Up: the playing track's album, with it highlighted.
                Key::Up => {
                    if let Some(&t) = queue.tracks.get(queue.at) {
                        let album = lib.tracks[t].album;
                        let place = lib.albums[album].tracks.iter().position(|&x| x == t);
                        self.stack.push(Level {
                            view: View::Album(album),
                            at: fixed_rows(View::Album(album)) + place.unwrap_or(0),
                        });
                    }
                    Outcome::None
                }
                // Down: the queue.
                Key::Down => {
                    let at = queue.at + fixed_rows(View::Queue);
                    self.stack.push(Level {
                        view: View::Queue,
                        at,
                    });
                    Outcome::None
                }
                k if k == back.0 => Outcome::Skip(-1),
                k if k == back.1 => Outcome::Skip(1),
                k if k == on.0 => Outcome::Seek(-10),
                k if k == on.1 => Outcome::Seek(10),
                _ => Outcome::None,
            };
        }
        let rows = rows(lib, queue, self.view());
        if rows.is_empty() {
            return Outcome::None;
        }
        let n = rows.len();
        let at = self.top().at.min(n - 1);
        // Settings change with Left, Right or A, as in the quick menu.
        if matches!(rows[at], Row::ScreenOff | Row::SkipWith | Row::ListSize) {
            match key {
                Key::Left => return Outcome::Change(rows[at], -1),
                Key::Right => return Outcome::Change(rows[at], 1),
                Key::A => return Outcome::Change(rows[at], 0),
                _ => {}
            }
        }
        // In the queue, X removes and the shoulders move a track, the cursor following.
        if let Row::Queued(i) = rows[at] {
            match key {
                Key::X => return Outcome::Remove(i),
                Key::L1 | Key::R1 => {
                    let by = if key == Key::L1 { -1 } else { 1 };
                    let to = i as i64 + by as i64;
                    if to >= 0 && (to as usize) < queue.tracks.len() {
                        self.top_mut().at = (at as i64 + by as i64) as usize;
                    }
                    return Outcome::Move(i, by);
                }
                _ => {}
            }
        }
        let at = match key {
            Key::Up => (at + n - 1) % n,
            Key::Down => (at + 1) % n,
            Key::Left => at.saturating_sub(per_page(self.view(), settings)),
            Key::Right => (at + per_page(self.view(), settings)).min(n - 1),
            Key::L1 => letter(lib, &rows, at, false),
            Key::R1 => letter(lib, &rows, at, true),
            // X adds the row's tracks to the end of the queue, Y puts them next.
            Key::X | Key::Y => {
                return match (self.tracks(lib, rows[at]), key) {
                    (Some(tracks), Key::X) => Outcome::Enqueue(tracks),
                    (Some(tracks), _) => Outcome::PlayNext(tracks),
                    (None, _) => Outcome::None,
                }
            }
            _ => return self.open(lib, queue, rows[at], at),
        };
        self.top_mut().at = at;
        // A letter jump names the letter it landed on.
        match key {
            Key::L1 | Key::R1 => {
                name(lib, rows[at]).map_or(Outcome::None, |n| Outcome::Letter(initial(n)))
            }
            _ => Outcome::None,
        }
    }

    pub fn page(&self, lib: &Library, queue: Queue, settings: &Settings) -> Page {
        let view = self.view();
        let rows = rows(lib, queue, view);
        let at = self.top().at.min(rows.len().saturating_sub(1));
        let per = per_page(view, settings);
        let page = at / per;
        let shown = || rows.iter().skip(page * per).take(per);
        Page {
            title: title(lib, view),
            rows: shown().map(|&r| label(lib, queue, view, r)).collect(),
            values: shown().map(|&r| value(lib, settings, r)).collect(),
            selected: at % per,
            actions: fixed_rows(view).saturating_sub(page * per).min(per),
            page,
            pages: rows.len().div_ceil(per).max(1),
            item: at.checked_sub(fixed_rows(view)).map(|i| i + 1),
            items: rows.len() - fixed_rows(view),
            per_page: per,
        }
    }

    /// Opens `view`, or goes back if it is open.
    fn visit(&mut self, view: View) {
        match self.view() {
            v if v == view && self.stack.len() > 1 => {
                self.stack.pop();
            }
            v if v == view => {}
            _ => self.stack.push(Level { view, at: 0 }),
        }
    }

    /// The tracks a row stands for.
    fn tracks(&self, lib: &Library, row: Row) -> Option<Vec<usize>> {
        Some(match row {
            Row::Artist(a) => lib.artist_tracks(a),
            Row::Album(a) => lib.albums[a].tracks.clone(),
            Row::Track(t) => vec![t],
            Row::PlayAll | Row::Shuffle => match self.view() {
                View::Artist(a) => lib.artist_tracks(a),
                View::Album(a) => lib.albums[a].tracks.clone(),
                View::Tracks => lib.tracks_by_title.clone(),
                View::Artists => (0..lib.tracks.len()).collect(),
                View::Albums => lib
                    .albums_by_name
                    .iter()
                    .flat_map(|&a| lib.albums[a].tracks.iter().copied())
                    .collect(),
                _ => return None,
            },
            _ => return None,
        })
    }

    fn open(&mut self, lib: &Library, queue: Queue, row: Row, at: usize) -> Outcome {
        let (view, outcome) = match row {
            Row::NowPlaying => (View::NowPlaying, Outcome::None),
            Row::Artists => (View::Artists, Outcome::None),
            Row::Albums => (View::Albums, Outcome::None),
            Row::Tracks => (View::Tracks, Outcome::None),
            Row::Rescan => return Outcome::Rescan,
            Row::ClearQueue => return Outcome::ClearUpNext,
            Row::ScreenOff | Row::SkipWith | Row::ListSize => return Outcome::Change(row, 0),
            Row::Settings => (View::Settings, Outcome::None),
            Row::Artist(a) => (View::Artist(a), Outcome::None),
            Row::Album(a) => (View::Album(a), Outcome::None),
            // A pick in the queue returns to Now Playing, which opened it.
            Row::Queued(i) if self.under_now_playing() => {
                self.stack.pop();
                return Outcome::Jump(i);
            }
            Row::Queued(i) => (View::NowPlaying, Outcome::Jump(i)),
            Row::PlayAll | Row::Shuffle => {
                let Some(tracks) = self.tracks(lib, row) else {
                    return Outcome::None;
                };
                // Shuffle deals what the list shows: albums or artists whole, or tracks.
                let shuffle = match (row, groups(lib, self.view())) {
                    (Row::PlayAll, _) => Shuffle::Off,
                    (_, Some(sizes)) => Shuffle::Groups(sizes),
                    (_, None) => Shuffle::Tracks,
                };
                let play = Outcome::Play {
                    tracks,
                    start: 0,
                    shuffle,
                };
                (View::NowPlaying, play)
            }
            Row::Track(_) => {
                let tracks = match self.view() {
                    View::Album(a) => lib.albums[a].tracks.clone(),
                    View::Tracks => lib.tracks_by_title.clone(),
                    _ => return Outcome::None,
                };
                let start = at - fixed_rows(self.view());
                let play = Outcome::Play {
                    tracks,
                    start,
                    shuffle: Shuffle::Off,
                };
                (View::NowPlaying, play)
            }
        };
        // The queue opens on what is playing.
        let at = match view {
            View::Queue => queue.at + fixed_rows(view),
            _ => 0,
        };
        self.stack.push(Level { view, at });
        outcome
    }

    fn under_now_playing(&self) -> bool {
        let n = self.stack.len();
        n >= 2 && self.stack[n - 2].view == View::NowPlaying
    }

    fn top(&self) -> &Level {
        self.stack.last().expect("the main screen is never popped")
    }

    fn top_mut(&mut self) -> &mut Level {
        self.stack
            .last_mut()
            .expect("the main screen is never popped")
    }
}

/// How a list's tracks split into the albums or artists it shows.
fn groups(lib: &Library, view: View) -> Option<Vec<usize>> {
    let albums =
        |ids: &mut dyn Iterator<Item = usize>| ids.map(|a| lib.albums[a].tracks.len()).collect();
    match view {
        View::Artists => Some(
            lib.artists
                .iter()
                .map(|a| a.albums.iter().map(|&b| lib.albums[b].tracks.len()).sum())
                .collect(),
        ),
        View::Albums => Some(albums(&mut lib.albums_by_name.iter().copied())),
        View::Artist(a) => Some(albums(&mut lib.artists[a].albums.iter().copied())),
        _ => None,
    }
}

/// Rows a page of `view` holds: menus keep slot's size, lists follow List Size.
pub fn per_page(view: View, settings: &Settings) -> usize {
    match view {
        View::Main | View::Settings => MENU_ROWS,
        _ => settings.list_size.page(),
    }
}

/// The action rows at the top of a screen.
fn fixed(view: View) -> &'static [Row] {
    match view {
        View::Artists | View::Albums | View::Tracks | View::Artist(_) | View::Album(_) => {
            &[Row::PlayAll, Row::Shuffle]
        }
        View::Queue => &[Row::ClearQueue],
        _ => &[],
    }
}

fn fixed_rows(view: View) -> usize {
    fixed(view).len()
}

pub fn rows(lib: &Library, queue: Queue, view: View) -> Vec<Row> {
    let mut rows = fixed(view).to_vec();
    match view {
        View::Main => rows.extend([
            Row::NowPlaying,
            Row::Artists,
            Row::Albums,
            Row::Tracks,
            Row::Settings,
        ]),
        View::Settings => rows.extend([Row::ScreenOff, Row::SkipWith, Row::ListSize, Row::Rescan]),
        View::Queue => rows.extend((0..queue.tracks.len()).map(Row::Queued)),
        View::Artists => rows.extend((0..lib.artists.len()).map(Row::Artist)),
        View::Artist(a) => rows.extend(lib.artists[a].albums.iter().map(|&a| Row::Album(a))),
        View::Albums => rows.extend(lib.albums_by_name.iter().map(|&a| Row::Album(a))),
        View::Album(a) => rows.extend(lib.albums[a].tracks.iter().map(|&t| Row::Track(t))),
        View::Tracks => rows.extend(lib.tracks_by_title.iter().map(|&t| Row::Track(t))),
        View::NowPlaying => {}
    }
    rows
}

fn title(lib: &Library, view: View) -> String {
    match view {
        View::Main => "Music".into(),
        View::Queue => "Queue".into(),
        View::Settings => "Music Settings".into(),
        View::Artists => "Artists".into(),
        View::Artist(a) => lib.artists[a].name.clone(),
        View::Albums => "Albums".into(),
        View::Album(a) => lib.albums[a].name.clone(),
        View::Tracks => "Tracks".into(),
        View::NowPlaying => "Now Playing".into(),
    }
}

/// The playing track shows the play symbol in place of its number.
fn label(lib: &Library, queue: Queue, view: View, row: Row) -> RowText {
    let mut text = describe(lib, queue, view, row);
    if text.playing {
        text.icon = Some(RowIcon::Play);
        text.before.clear();
    }
    text.action = fixed(view).contains(&row);
    text
}

fn describe(lib: &Library, queue: Queue, view: View, row: Row) -> RowText {
    let current = queue.tracks.get(queue.at).copied();
    let track = |t: usize| RowText {
        text: lib.tracks[t].title.clone(),
        after: lib.artist_of(t).name.clone(),
        playing: Some(t) == current,
        ..RowText::default()
    };
    let plain = |text: &str| RowText::plain(text);
    match row {
        // As Rockbox does: Resume while paused, dim with nothing to resume.
        Row::NowPlaying => match (queue.tracks.is_empty(), queue.playing) {
            (true, _) => RowText {
                dim: true,
                ..plain("Now Playing")
            },
            (false, true) => plain("Now Playing"),
            (false, false) => plain("Resume"),
        },
        Row::Artists => plain("Artists"),
        Row::Albums => plain("Albums"),
        Row::Tracks => plain("Tracks"),
        Row::Rescan => plain("Rescan Library"),
        Row::ClearQueue => plain("Clear Up Next"),
        Row::Settings => plain("Settings"),
        Row::ScreenOff => plain("Screen Off"),
        Row::SkipWith => plain("Skip Tracks"),
        Row::ListSize => plain("List Size"),
        Row::PlayAll => plain("Play All"),
        Row::Shuffle => plain(match view {
            View::Artists => "Shuffle Artists",
            View::Albums | View::Artist(_) => "Shuffle Albums",
            _ => "Shuffle Tracks",
        }),
        Row::Artist(a) => plain(&lib.artists[a].name),
        // All albums name their artist; an artist's own do not need to.
        Row::Album(a) if view == View::Albums => RowText {
            after: lib.artists[lib.albums[a].artist].name.clone(),
            ..plain(&lib.albums[a].name)
        },
        Row::Album(a) => plain(&lib.albums[a].name),
        // Tracks show their number in their album, their artist elsewhere.
        Row::Track(t) if view == View::Album(lib.tracks[t].album) => RowText {
            before: lib.tracks[t].number.clone(),
            after: String::new(),
            ..track(t)
        },
        Row::Track(t) => track(t),
        Row::Queued(i) => RowText {
            playing: i == queue.at,
            dim: i < queue.at,
            ..track(queue.tracks[i])
        },
    }
}

/// A setting's choice, or a main-screen count.
fn value(lib: &Library, settings: &Settings, row: Row) -> Option<String> {
    let count = |n: usize| (n > 0).then(|| n.to_string());
    let pick = |on: bool, yes: &str, no: &str| Some(if on { yes } else { no }.to_string());
    match row {
        Row::Artists => count(lib.artists.len()),
        Row::Albums => count(lib.albums.len()),
        Row::Tracks => count(lib.tracks.len()),
        Row::ScreenOff => pick(settings.play_screen_off, "Keep Playing", "Pause"),
        Row::SkipWith => pick(settings.skip_with_dpad, "D-pad", "L1 R1"),
        Row::ListSize => Some(settings.list_size.label().into()),
        _ => None,
    }
}

fn name(lib: &Library, row: Row) -> Option<&str> {
    match row {
        Row::Artist(a) => Some(&lib.artists[a].name),
        Row::Album(a) => Some(&lib.albums[a].name),
        Row::Track(t) => Some(&lib.tracks[t].title),
        _ => None,
    }
}

/// The first row of the previous or next letter, grouped as on the shelf. Action rows are
/// their own group.
fn letter(lib: &Library, rows: &[Row], at: usize, forward: bool) -> usize {
    let group = |i: usize| name(lib, rows[i]).map(initial);
    let here = group(at);
    let start = |mut i: usize| {
        while i > 0 && group(i - 1) == group(i) {
            i -= 1;
        }
        i
    };
    match forward {
        true => (at + 1..rows.len())
            .find(|&i| group(i) != here)
            .unwrap_or(at),
        false => match start(at) {
            0 => 0,
            first => start(first - 1),
        },
    }
}
