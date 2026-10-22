//! The card's files as the store reads and writes them: `key = value` text a person may edit and
//! comment, and a file replaced whole or not at all. The player's own, so it needs no frontend.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Each `key = value` line of `text`; comments (`#`, `;`), sections and blank lines left out.
pub fn parse(text: &str) -> HashMap<String, String> {
    let lines = text.lines().filter_map(entry);
    lines.map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn entry(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(['#', ';', '[']) {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty()).then_some((key, value.trim()))
}

/// `text` with `key` set to `value`: its line replaced where it was, the first if twice, or added
/// at the end; every other line as it was. A value that would not read back as itself, as one
/// with a line break, is refused.
pub fn set(text: &str, key: &str, value: &str) -> std::io::Result<String> {
    let line = format!("{key} = {value}");
    if line.lines().count() != 1 || entry(&line) != Some((key, value)) {
        let e = format!("cannot hold the entry {line:?}");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, e));
    }
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    let mut set = false;
    for was in text.lines() {
        let this = entry(was).is_some_and(|(k, _)| k == key);
        match (this, set) {
            (false, _) => out.push_str(was),
            (true, false) => out.push_str(&line),
            (true, true) => continue,
        }
        set |= this;
        out.push('\n');
    }
    if !set {
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Writes `bytes` to `path` whole or not at all: to a file beside it first, flushed to the card,
/// then put in its place, so a power cut never leaves it cut short.
pub fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let part = beside(path);
    let written = (|| {
        let mut file = std::fs::File::create(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&part, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written?;
    // The rename, kept too.
    if let Some(dir) = path.parent().and_then(|d| std::fs::File::open(d).ok()) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// The file `path` is written to before it takes its place: hidden, beside it.
fn beside(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.part"))
}

/// Whether `path` is hidden, as a dot names it: left out of the library and the playlists.
pub fn is_hidden(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str());
    name.is_some_and(|n| n.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_s_keys_are_read_and_its_comments_and_sections_left_out() {
        let read =
            parse("# a note\n; another\n[section]\nrepeat = all\n  eq =  Vocal \nbad\n= x\n");
        assert_eq!(read.len(), 2);
        assert_eq!(
            (read["repeat"].as_str(), read["eq"].as_str()),
            ("all", "Vocal")
        );
    }

    #[test]
    fn a_key_set_replaces_its_line_where_it_was_and_leaves_the_rest_as_written() {
        let was = "# mine\nrepeat = off\neq = Vocal\nrepeat = one\n";
        assert_eq!(
            set(was, "repeat", "all").unwrap(),
            "# mine\nrepeat = all\neq = Vocal\n"
        );
        assert_eq!(
            set("", "jump", "15").unwrap(),
            "jump = 15\n",
            "added when new"
        );
        assert!(
            set(was, "eq", "a\nb").is_err(),
            "a line break would not read back"
        );
    }

    #[test]
    fn a_file_is_written_whole_and_nothing_is_left_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("music.txt");
        write_whole(&path, b"one").unwrap();
        write_whole(&path, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no part left"
        );
        assert!(is_hidden(&beside(&path)) && !is_hidden(&path));
    }
}
