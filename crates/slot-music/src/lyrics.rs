//! A song's lyrics: an `.lrc` file beside it, as players look for first, or else its tags'.
//! Timed lyrics are LRC's: `[mm:ss.xx]` before each line, so the line sung can be followed;
//! anything else is plain text, to read.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;

/// One line, and when it is sung, in seconds; none in plain lyrics. An empty line is a pause.
#[derive(Clone, PartialEq, Debug)]
pub struct Line {
    pub at: Option<f64>,
    pub text: String,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

impl Lyrics {
    /// `text` as LRC, if any of its lines is timed; else as plain lines, the blank ones at
    /// either end left out.
    pub fn parse(text: &str) -> Lyrics {
        let mut offset = 0.0;
        let mut timed = Vec::new();
        for raw in text.lines() {
            let (stamps, words) = stamps(raw.trim());
            match (stamps.as_slice(), tag(raw.trim(), "offset")) {
                // In milliseconds; a later offset shows each line sooner.
                ([], Some(ms)) => offset = ms.trim().parse::<f64>().unwrap_or(0.0) / 1000.0,
                ([], _) => {}
                (at, _) => timed.extend(at.iter().map(|&at| (at, words.trim().to_string()))),
            }
        }
        if !timed.is_empty() {
            timed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let lines = timed.into_iter().map(|(at, text)| Line {
                at: Some((at - offset).max(0.0)),
                text,
            });
            return Lyrics {
                lines: lines.collect(),
            };
        }
        let cleaned = clean(text);
        let lines: Vec<&str> = cleaned.lines().map(str::trim_end).collect();
        let first = lines.iter().position(|l| !l.trim().is_empty());
        let last = lines.iter().rposition(|l| !l.trim().is_empty());
        let (Some(first), Some(last)) = (first, last) else {
            return Lyrics::default();
        };
        let plain = lines[first..=last].iter().map(|&text| Line {
            at: None,
            text: text.trim().to_string(),
        });
        Lyrics {
            lines: plain.collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn timed(&self) -> bool {
        self.lines.first().is_some_and(|l| l.at.is_some())
    }

    /// The line sung `at` seconds in: the last one begun; none before the first, or in plain
    /// lyrics.
    pub fn current(&self, at: f64) -> Option<usize> {
        self.lines
            .iter()
            .rposition(|l| l.at.is_some_and(|t| t <= at))
    }
}

/// The lyrics for the song at `path`: `name.lrc` beside it, or its tags'; none when neither
/// has any.
pub fn read(path: &Path) -> Option<Lyrics> {
    let beside = std::fs::read_to_string(path.with_extension("lrc")).ok();
    let text = beside.or_else(|| tagged(path))?;
    Some(Lyrics::parse(&text)).filter(|l| !l.is_empty())
}

/// The lyrics in `path`'s tags: ID3's unsynchronised lyrics, or a `LYRICS` field.
fn tagged(path: &Path) -> Option<String> {
    let options = ParseOptions::new()
        .read_cover_art(false)
        .read_properties(false);
    let tagged = Probe::open(path).ok()?.options(options).read().ok()?;
    let tag = tagged.primary_tag().or(tagged.first_tag())?;
    let text = tag.get_string(ItemKey::UnsyncLyrics);
    text.or_else(|| tag.get_string(ItemKey::Lyrics))
        .map(str::to_string)
}

/// Plain lyrics without what scraping them from Genius leaves, as beets does: a first line
/// "35 ContributorsSong Lyrics…", "You might also like" in the middle, and "…Embed" at the end.
fn clean(text: &str) -> String {
    let mut text = text.replace("You might also like", "");
    if let Some((head, rest)) = text.split_once('\n') {
        let header = head
            .split_once("Contributors")
            .filter(|(n, _)| n.trim().parse::<u32>().is_ok());
        if let Some((_, title)) = header.and_then(|(_, t)| t.split_once("Lyrics")) {
            text = format!("{}\n{rest}", title.trim());
        }
    }
    let trimmed = text.trim_end();
    let embed = trimmed
        .strip_suffix("Embed")
        .map(|t| t.trim_end_matches(|c: char| c.is_ascii_digit()));
    embed.unwrap_or(trimmed).to_string()
}

/// The times a line starts with, `[mm:ss]`, `[mm:ss.xx]` or `[mm:ss:xx]`, and what follows.
fn stamps(line: &str) -> (Vec<f64>, &str) {
    let mut times = Vec::new();
    let mut rest = line;
    while let Some(inner) = rest.strip_prefix('[') {
        let Some((stamp, after)) = inner.split_once(']') else {
            break;
        };
        let Some(at) = time(stamp) else {
            break;
        };
        times.push(at);
        rest = after;
    }
    (times, rest)
}

fn time(stamp: &str) -> Option<f64> {
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes: f64 = minutes.trim().parse().ok()?;
    // Some write hundredths after a second colon.
    let seconds = seconds.replacen(':', ".", 1);
    let seconds: f64 = seconds.trim().parse().ok()?;
    (seconds < 60.0).then_some(minutes * 60.0 + seconds)
}

/// A metadata tag's value, `[name:value]`.
fn tag<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = inner.split_once(':')?;
    key.trim().eq_ignore_ascii_case(name).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lyrics: &Lyrics) -> Vec<&str> {
        lyrics.lines.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn lrc_is_timed_line_by_line_in_order_with_its_tags_left_out() {
        let lyrics = Lyrics::parse(
            "[ar:AC/DC]\n[ti:T.N.T.]\n[00:21.74] See me ride out\n[00:07.81] Oi, oi\n\
             [00:21.70]\n[01:02:50]Last\n",
        );
        assert!(lyrics.timed());
        assert_eq!(texts(&lyrics), ["Oi, oi", "", "See me ride out", "Last"]);
        assert_eq!(lyrics.lines[0].at, Some(7.81));
        assert_eq!(lyrics.lines[3].at, Some(62.5), "hundredths after a colon");
    }

    #[test]
    fn a_line_sung_twice_is_where_each_time_says_and_the_offset_moves_them_all() {
        let lyrics = Lyrics::parse("[offset:500]\n[00:10.00][00:30.00]Chorus\n[00:20.00]Verse");
        let at: Vec<Option<f64>> = lyrics.lines.iter().map(|l| l.at).collect();
        assert_eq!(at, [Some(9.5), Some(19.5), Some(29.5)]);
        assert_eq!(texts(&lyrics), ["Chorus", "Verse", "Chorus"]);
    }

    #[test]
    fn the_line_sung_is_the_last_begun() {
        let lyrics = Lyrics::parse("[00:05.00]One\n[00:10.00]Two");
        assert_eq!(lyrics.current(2.0), None, "before the first");
        assert_eq!(lyrics.current(5.0), Some(0));
        assert_eq!(lyrics.current(60.0), Some(1));
    }

    #[test]
    fn plain_lyrics_are_lines_to_read_without_the_blank_ends() {
        let lyrics = Lyrics::parse("\n\nWhen the snow starts to fall\n\nTides will turn  \n\n");
        assert!(!lyrics.timed());
        assert_eq!(
            texts(&lyrics),
            ["When the snow starts to fall", "", "Tides will turn"]
        );
        assert_eq!(lyrics.current(30.0), None);
        assert!(Lyrics::parse("\n  \n").is_empty());
    }

    #[test]
    fn what_scraping_leaves_in_plain_lyrics_is_left_out() {
        let lyrics = Lyrics::parse(
            "35 ContributorsTake a Look Around Lyrics[Intro]\n\"Mr. Durst?\"\nYou might also like\nThe end12Embed",
        );
        assert_eq!(texts(&lyrics), ["[Intro]", "\"Mr. Durst?\"", "", "The end"]);
    }

    #[test]
    fn an_lrc_beside_the_song_is_read_first() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("a.mp3");
        std::fs::write(&song, b"not a song").unwrap();
        assert_eq!(read(&song), None, "no file beside it, no tags");
        std::fs::write(dir.path().join("a.lrc"), "[00:01.00]Hello").unwrap();
        assert_eq!(
            read(&song).map(|l| texts(&l).join("")),
            Some("Hello".into())
        );
    }
}
