//! AGENTS.md's rules, checked on the code itself, so a change that breaks one fails here rather
//! than in review: where words, colours and motion live, that the music's state never reads a
//! clock, that threads share no state but by message, and which crate may use which.
//!
//! Each rule reads our source files, without their tests and comments, and lists every line
//! that breaks it. A line allowed to is named here, whole, with why.

use std::path::{Path, PathBuf};

/// The crates' folder.
fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Every `.rs` file under `dir`, from the crates' folder.
fn sources(dir: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") && !is_tests(&path) {
                out.push(path);
            }
        }
    }
    let root = crates().join(dir);
    if root.is_file() {
        return vec![root];
    }
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

/// A file of tests, or a folder of them: tests, as the rest's are, not the code.
fn is_tests(path: &Path) -> bool {
    path.ends_with("tests.rs") || path.components().any(|c| c.as_os_str() == "tests")
}

/// A file's code lines, numbered from 1: up to its tests, which come last, and without comments.
fn code(path: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .enumerate()
        .take_while(|(_, line)| line.trim() != "#[cfg(test)]")
        .map(|(i, line)| (i + 1, line.trim().to_string()))
        .filter(|(_, line)| !line.starts_with("//"))
        .collect()
}

/// A file's name from the crates' folder, as the allowed lists name it.
fn name(path: &Path) -> String {
    let name = path.strip_prefix(crates()).unwrap_or(path);
    name.to_string_lossy().replace('\\', "/")
}

/// Every line of the files under `dirs`, but `except`, where `breaks` holds and that `allowed`
/// does not name: as "file:line: code".
fn breaches(
    dirs: &[&str],
    except: &[&str],
    breaks: impl Fn(&str) -> bool,
    allowed: &[(&str, &str)],
) -> Vec<String> {
    let mut found = Vec::new();
    for dir in dirs {
        for path in sources(dir) {
            let file = name(&path);
            if except.iter().any(|e| file.ends_with(e)) {
                continue;
            }
            for (n, line) in code(&path) {
                let allow = allowed.iter().any(|(f, l)| file.ends_with(f) && line == *l);
                if breaks(&line) && !allow {
                    found.push(format!("{file}:{n}: {line}"));
                }
            }
        }
    }
    found
}

#[test]
fn words_are_text_rs_s() {
    // A literal that starts as a word does, a capital then small letters, and is no path.
    let word = |line: &str| {
        line.split('"').skip(1).step_by(2).any(|s| {
            let mut c = s.chars();
            if s.contains('/') {
                return false;
            }
            c.next().is_some_and(char::is_uppercase) && c.next().is_some_and(char::is_lowercase)
        })
    };
    let allowed = [
        // The folders on the card, not words shown.
        ("library/mod.rs", r#"pub const MUSIC_DIR: &str = "Music";"#),
        (
            "library/mod.rs",
            r#"pub const BOOKS_DIR: &str = "Audiobooks";"#,
        ),
        (
            "store/playlists.rs",
            r#"pub const PLAYLISTS_DIR: &str = "Playlists";"#,
        ),
        // The Favorites playlist's file, which other players read; the row says text.rs's.
        (
            "store/playlists.rs",
            r#"const FAVORITES: &str = "Favorites";"#,
        ),
        // What lyrics sites add to lyrics, found in the files and taken out.
        (
            "lyrics.rs",
            r#"let mut text = text.replace("You might also like", "");"#,
        ),
        ("lyrics.rs", r#".split_once("Contributors")"#),
        (
            "lyrics.rs",
            r#"if let Some((_, title)) = header.and_then(|(_, t)| t.split_once("Lyrics")) {"#,
        ),
        ("lyrics.rs", r#".strip_suffix("Embed")"#),
    ];
    // The music's, its kit's and its glue's: what slot's own screens say is slot's.
    let dirs = [
        "slot-music/src",
        "slot-ui/src/music",
        "slot/src/music_faces",
        "slot/src/app/music.rs",
    ];
    let found = breaches(&dirs, &["text.rs"], word, &allowed);
    assert!(
        found.is_empty(),
        "words outside text.rs:\n{}",
        found.join("\n")
    );
}

#[test]
fn colours_are_theme_rs_s() {
    let colour = |line: &str| line.contains("[0x");
    let dirs = ["slot-ui/src/music", "slot/src/music_faces"];
    let found = breaches(&dirs, &["theme.rs"], colour, &[]);
    assert!(
        found.is_empty(),
        "colours outside theme.rs:\n{}",
        found.join("\n")
    );
}

#[test]
fn motion_is_tween_rs_s() {
    let time = |line: &str| line.contains("Duration::from_");
    let dirs = ["slot-ui/src/music", "slot/src/music_faces"];
    let found = breaches(&dirs, &["tween.rs"], time, &[]);
    assert!(
        found.is_empty(),
        "durations outside tween.rs:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_music_s_state_never_reads_a_clock() {
    let clock = |line: &str| {
        ["Instant::now", "SystemTime::now", ".elapsed()"]
            .iter()
            .any(|c| line.contains(c))
    };
    let allowed = [
        // The shell's clock, the one the state is given.
        ("music/mod.rs", "at: Instant::now(),"),
        ("music/mod.rs", "let since = std::time::SystemTime::now()"),
        // The perf log's own timings: how long the frame took, not what it does.
        ("music/mod.rs", "let began = Instant::now();"),
        ("music/mod.rs", "perf.stats.poll.add(began.elapsed());"),
        ("screens/mod.rs", "let began = std::time::Instant::now();"),
        ("screens/mod.rs", "self.note_screen(began.elapsed());"),
        // The scan's thread, on the card: when a song came in, and how long the scan took.
        ("library/scan.rs", "let now = std::time::SystemTime::now()"),
        ("music/loading.rs", "let started = Instant::now();"),
        (
            "music/loading.rs",
            "log::info!(\"library read in {} ms\", started.elapsed().as_millis());",
        ),
    ];
    // The perf log's thread keeps its own time, apart from the state.
    let found = breaches(&["slot-music/src"], &["music/perf.rs"], clock, &allowed);
    assert!(
        found.is_empty(),
        "clocks read in the state:\n{}",
        found.join("\n")
    );
}

#[test]
fn threads_share_nothing_but_messages_and_one_snapshot() {
    let shared = |line: &str| {
        ["Atomic", "Condvar", "RwLock", "Mutex<"]
            .iter()
            .any(|s| line.contains(s))
    };
    let allowed = [
        // The player's status: one value the writer replaces whole.
        (
            "slot-player/src/engine.rs",
            "pub(crate) struct Shared(Mutex<Snapshot>);",
        ),
        (
            "slot-player/src/engine.rs",
            "fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {",
        ),
        // The scan's progress: one value, so the two halves never disagree.
        (
            "library/scan.rs",
            "pub struct Progress(Mutex<(usize, usize)>);",
        ),
        (
            "library/scan.rs",
            "fn lock(&self) -> std::sync::MutexGuard<'_, (usize, usize)> {",
        ),
    ];
    // The perf log's counters are statistics, a moment out with each other at worst.
    let dirs = ["slot-music/src", "slot-player/src"];
    let found = breaches(&dirs, &["music/perf.rs"], shared, &allowed);
    assert!(
        found.is_empty(),
        "state shared between threads:\n{}",
        found.join("\n")
    );
}

#[test]
fn threads_are_started_only_where_they_are_owned() {
    let spawn = |line: &str| line.contains("thread::Builder") || line.contains("thread::spawn");
    let owners = [
        "slot-player/src/lib.rs",
        "slot-player/src/engine.rs",
        "slot-music/src/store/mod.rs",
        "slot-music/src/music/worker.rs",
        "slot-music/src/music/loading.rs",
        "slot-music/src/music/perf.rs",
    ];
    let dirs = ["slot-music/src", "slot-player/src"];
    let found = breaches(&dirs, &owners, spawn, &[]);
    assert!(
        found.is_empty(),
        "a thread with no owner:\n{}",
        found.join("\n")
    );
}

#[test]
fn each_layer_knows_only_the_one_below() {
    // slot's own crates a crate depends on.
    let uses = |krate: &str| -> Vec<String> {
        let text = std::fs::read_to_string(crates().join(krate).join("Cargo.toml")).unwrap();
        let deps = text
            .split("[dependencies]")
            .nth(1)
            .and_then(|d| d.split("\n[").next())
            .unwrap_or("");
        deps.lines()
            .filter_map(|l| l.split(['=', ' ']).next())
            .filter(|d| d.starts_with("slot-"))
            .map(String::from)
            .collect()
    };
    assert_eq!(
        uses("slot-player"),
        Vec::<String>::new(),
        "the player knows no one"
    );
    assert_eq!(
        uses("slot-music"),
        ["slot-player", "slot-store"],
        "nor draws"
    );
    let ui = uses("slot-ui");
    assert!(
        !ui.iter().any(|d| d == "slot-music" || d == "slot-player"),
        "the kit draws and decides nothing: {ui:?}"
    );
}

#[test]
fn the_music_s_face_is_all_slot_sees_of_it() {
    // What lib.rs makes public, outside its tests: each name it exports.
    let lib = crates().join("slot-music/src/lib.rs");
    let code: String = code(&lib).into_iter().map(|(_, l)| l + " ").collect();
    let mut exported: Vec<String> = code
        .split(';')
        .map(str::trim)
        .filter(|s| s.starts_with("pub use ") || s.starts_with("pub mod "))
        .flat_map(|s| {
            let names = s.rsplit("::").next().unwrap_or(s);
            let names = names.trim_start_matches("pub mod ");
            names
                .split([',', '{', '}'])
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();
    exported.sort();
    // The music itself, the buttons in, the screen out and its parts, the picture and the words. And
    // the player it plays through, which the frontend starts, so the music knows no device.
    let face = [
        "AlbumId",
        "Art",
        "Body",
        "Button",
        "Handle",
        "Hold",
        "Input",
        "Legend",
        "ListBody",
        "MainHold",
        "Music",
        "NowPlayingBody",
        "Repeat",
        "RowText",
        "Screen",
        "Status",
        "Target",
        "TextBody",
        "text",
    ];
    assert_eq!(
        exported, face,
        "lib.rs's face: a new name here is a new boundary, said so"
    );
}

#[test]
fn the_log_is_told_one_way() {
    // `log::warn!` for what went wrong, `log::info!` for what happens, through the one logger.
    let prints = |line: &str| line.contains("eprintln!") || line.contains("println!");
    let owners = ["slot-music/src/logging.rs", "slot-player/src/bench.rs"];
    let dirs = [
        "slot-music/src",
        "slot-player/src",
        "slot-ui/src/music",
        "slot/src/music_faces",
    ];
    let mut found = breaches(&dirs, &owners, prints, &[]);
    let glue = ["slot/src/app/music.rs", "slot/src/audio/line_out.rs"];
    for file in glue {
        let text = std::fs::read_to_string(crates().join(file)).unwrap();
        found.extend(
            text.lines()
                .filter(|l| prints(l))
                .map(|l| format!("{file}: {l}")),
        );
    }
    assert!(
        found.is_empty(),
        "printed, not logged:\n{}",
        found.join("\n")
    );
}
