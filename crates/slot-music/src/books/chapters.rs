//! A book's chapters read from its file, as players show them before it plays: an MP4's
//! (an .m4b's) QuickTime chapter track, or else Nero's chapter list; or an MP3's ID3 `CHAP`
//! frames, as podcasts and audiobook tools write them.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The file's metadata box is read whole; one larger than this is not a book's.
const MOOV_MAX: u64 = 16 << 20;
/// More chapters than this are not a book's: a file that says so is read no further.
const CHAPTERS_MAX: usize = 10_000;

/// A chapter of a file: where it starts, in seconds, and its title, which may be empty.
#[derive(Clone, PartialEq, Debug)]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

/// The file's chapters, in order; none when it names none.
pub fn read(path: &Path) -> Vec<Chapter> {
    let id3 = File::open(path).ok().and_then(|mut f| id3(&mut f));
    let marks = id3.unwrap_or_else(|| {
        let mut file = File::open(path).ok();
        let moov = file.as_mut().and_then(moov).unwrap_or_default();
        let track = file.and_then(|mut f| track(&mut f, &moov));
        track.or_else(|| chpl(&moov)).unwrap_or_default()
    });
    let chapter = |(start, title)| Chapter { start, title };
    marks.into_iter().map(chapter).collect()
}

/// The chapters an ID3v2 tag at the start of the file names, by their `CHAP` frames: each
/// one's start, in milliseconds, and its title, from the `TIT2` frame inside it. None when the
/// file has no tag, or no chapters in it.
fn id3(file: &mut File) -> Option<Vec<(f64, String)>> {
    let mut head = [0; 10];
    file.read_exact(&mut head).ok()?;
    let (version, flags) = (head[3], head[5]);
    // Unsynchronised tags are rare, and not worth undoing for chapters.
    if &head[..3] != b"ID3" || !(3..=4).contains(&version) || flags & 0x80 != 0 {
        return None;
    }
    let size = syncsafe(&head[6..10])? as u64;
    let mut tag = vec![0; size.min(MOOV_MAX) as usize];
    file.read_exact(&mut tag).ok()?;
    let mut frames = &tag[..];
    if flags & 0x40 != 0 {
        // An extended header: its size counts itself in version 4, not in version 3.
        let size = match version {
            4 => syncsafe(frames.get(..4)?)? as usize,
            _ => u32_at(frames, 0)? as usize + 4,
        };
        frames = frames.get(size..)?;
    }
    let mut chapters: Vec<(f64, String)> = id3_frames(frames, version)
        .filter(|(id, _)| id == b"CHAP")
        .filter_map(|(_, body)| {
            // The element's name, ended by a zero; then its start and end, in milliseconds,
            // and their byte offsets; then frames of its own.
            let name = body.iter().position(|&b| b == 0)? + 1;
            let start = u32_at(body, name)?;
            let inner = body.get(name + 16..)?;
            let title = id3_frames(inner, version)
                .find(|(id, _)| id == b"TIT2")
                .map(|(_, text)| id3_text(text))
                .unwrap_or_default();
            Some((f64::from(start) / 1000.0, title))
        })
        .collect();
    chapters.sort_by(|a, b| a.0.total_cmp(&b.0));
    (!chapters.is_empty()).then_some(chapters)
}

/// The frames in `data`, each one's id and body; version 4 sizes them in seven bits a byte.
fn id3_frames(mut data: &[u8], version: u8) -> impl Iterator<Item = (&[u8], &[u8])> {
    std::iter::from_fn(move || {
        let id = data.get(..4)?;
        if id[0] == 0 {
            return None;
        }
        let raw = data.get(4..8)?;
        let size = match version {
            4 => syncsafe(raw)?,
            _ => u32::from_be_bytes(raw.try_into().ok()?),
        } as usize;
        let body = data.get(10..10 + size)?;
        data = &data[10 + size..];
        Some((id, body))
    })
}

/// Seven bits a byte, as ID3 keeps its sizes so they never read as a sync.
fn syncsafe(bytes: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(bytes.iter().fold(0, |n, &b| n << 7 | u32::from(b & 0x7f)))
}

/// A text frame's text: Latin-1, UTF-16 with a byte order mark, UTF-16 big-endian, or UTF-8,
/// as its first byte says, up to its end.
fn id3_text(frame: &[u8]) -> String {
    let (encoding, text) = frame.split_first().unwrap_or((&3, &[]));
    let units = |big: bool, bytes: &[u8]| -> Vec<u16> {
        let pair = |p: &[u8]| {
            if big {
                u16::from_be_bytes([p[0], p[1]])
            } else {
                u16::from_le_bytes([p[0], p[1]])
            }
        };
        bytes.chunks_exact(2).map(pair).collect()
    };
    let text = match encoding {
        0 => text.iter().map(|&b| char::from(b)).collect(),
        1 => match text {
            [0xfe, 0xff, rest @ ..] => String::from_utf16_lossy(&units(true, rest)),
            [0xff, 0xfe, rest @ ..] => String::from_utf16_lossy(&units(false, rest)),
            rest => String::from_utf16_lossy(&units(false, rest)),
        },
        2 => String::from_utf16_lossy(&units(true, text)),
        _ => String::from_utf8_lossy(text).into_owned(),
    };
    text.trim_end_matches('\0').trim().to_string()
}

/// The boxes in `data`: each one's kind and what it holds.
fn boxes(mut data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    std::iter::from_fn(move || {
        let size = u32::from_be_bytes(data.get(..4)?.try_into().ok()?) as usize;
        let (kind, body) = (data.get(4..8)?, data.get(8..size.min(data.len()))?);
        data = &data[size.max(8).min(data.len())..];
        Some((kind, body))
    })
}

/// The box of `kind` at `path` under `data`.
fn find<'a>(data: &'a [u8], path: &[&[u8]]) -> Option<&'a [u8]> {
    path.iter().try_fold(data, |data, kind| {
        boxes(data).find(|(k, _)| k == kind).map(|(_, body)| body)
    })
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// Where `tkhd` keeps its track and `mdhd` its time scale: past 64-bit times in version 1.
fn version_at(header: &[u8]) -> Option<usize> {
    Some(if *header.first()? == 1 { 20 } else { 12 })
}

/// The `moov` box, wherever it is among the file's top boxes.
fn moov(file: &mut File) -> Option<Vec<u8>> {
    let mut at = 0;
    loop {
        let mut head = [0; 16];
        file.seek(SeekFrom::Start(at)).ok()?;
        file.read_exact(&mut head[..8]).ok()?;
        let (size, moov) = (u32_at(&head, 0)? as u64, &head[4..8] == b"moov");
        let (size, skip) = match size {
            1 => {
                file.read_exact(&mut head[8..]).ok()?;
                (u64_at(&head, 8)?, 16)
            }
            0 => (file.metadata().ok()?.len().checked_sub(at)?, 8),
            size => (size, 8),
        };
        if moov && size <= MOOV_MAX {
            let mut moov = vec![0; size.checked_sub(skip)? as usize];
            file.read_exact(&mut moov).ok()?;
            return Some(moov);
        }
        // A size past the end of the file ends the search, rather than come round again.
        at = at.checked_add(size.max(8))?;
    }
}

/// Nero's list, in `udta`: a count, then each start in 100 ns and its title.
fn chpl(moov: &[u8]) -> Option<Vec<(f64, String)>> {
    let chpl = find(moov, &[b"udta", b"chpl"])?;
    let mut at = if chpl.first()? == &1 { 8 } else { 4 };
    let n = *chpl.get(at)?;
    at += 1;
    let mut chapters = Vec::new();
    for _ in 0..n {
        let start = u64_at(chpl, at)? as f64 / 1e7;
        let len = *chpl.get(at + 8)? as usize;
        let title = chpl.get(at + 9..at + 9 + len)?;
        chapters.push((start, String::from_utf8_lossy(title).into_owned()));
        at += 9 + len;
    }
    (!chapters.is_empty()).then_some(chapters)
}

/// The text track the audio's `tref/chap` names: each sample is a chapter, its title a
/// 16-bit length and then the text, at the time the sample starts.
fn track(file: &mut File, moov: &[u8]) -> Option<Vec<(f64, String)>> {
    let traks = || boxes(moov).filter(|(k, _)| k == b"trak").map(|(_, b)| b);
    let id = traks().find_map(|t| u32_at(find(t, &[b"tref", b"chap"])?, 0))?;
    let trak = traks().find(|t| {
        let tkhd = find(t, &[b"tkhd"]);
        tkhd.and_then(|h| u32_at(h, version_at(h)?)) == Some(id)
    })?;
    let mdhd = find(trak, &[b"mdia", b"mdhd"])?;
    let scale = u32_at(mdhd, version_at(mdhd)?)?.max(1) as f64;
    let stbl = find(trak, &[b"mdia", b"minf", b"stbl"])?;
    let starts = starts(find(stbl, &[b"stts"])?)?;
    let offsets = offsets(stbl, starts.len())?;
    let mut chapters = Vec::new();
    for (start, offset) in starts.into_iter().zip(offsets) {
        let mut len = [0; 2];
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read_exact(&mut len).ok()?;
        let mut title = vec![0; u16::from_be_bytes(len) as usize];
        file.read_exact(&mut title).ok()?;
        chapters.push((
            start as f64 / scale,
            String::from_utf8_lossy(&title).into_owned(),
        ));
    }
    (!chapters.is_empty()).then_some(chapters)
}

/// When each sample starts, in the track's time scale, from the runs of their durations: at
/// most [`CHAPTERS_MAX`], whatever the runs say.
fn starts(stts: &[u8]) -> Option<Vec<u64>> {
    let mut starts = Vec::new();
    let mut t = 0;
    for run in 0..u32_at(stts, 4)? as usize {
        let (n, d) = (u32_at(stts, 8 + run * 8)?, u32_at(stts, 12 + run * 8)?);
        let left = CHAPTERS_MAX - starts.len();
        for _ in 0..(n as usize).min(left) {
            starts.push(t);
            t += u64::from(d);
        }
    }
    Some(starts)
}

/// Where each of the first `n` samples is in the file: chunks from `stco` or `co64`, how many
/// samples each holds from `stsc`, and their sizes from `stsz`.
fn offsets(stbl: &[u8], n: usize) -> Option<Vec<u64>> {
    let chunks: Vec<u64> = match (find(stbl, &[b"stco"]), find(stbl, &[b"co64"])) {
        (Some(c), _) => (0..u32_at(c, 4)? as usize)
            .map(|i| u32_at(c, 8 + i * 4).map(u64::from))
            .collect::<Option<_>>()?,
        (_, Some(c)) => (0..u32_at(c, 4)? as usize)
            .map(|i| u64_at(c, 8 + i * 8))
            .collect::<Option<_>>()?,
        _ => return None,
    };
    let stsc = find(stbl, &[b"stsc"])?;
    let runs: Vec<(usize, u32)> = (0..u32_at(stsc, 4)? as usize)
        .map(|i| {
            Some((
                u32_at(stsc, 8 + i * 12)? as usize,
                u32_at(stsc, 12 + i * 12)?,
            ))
        })
        .collect::<Option<_>>()?;
    let stsz = find(stbl, &[b"stsz"])?;
    let size = |i: usize| match u32_at(stsz, 4)? {
        0 => u32_at(stsz, 12 + i * 4),
        same => Some(same),
    };
    let mut offsets = Vec::with_capacity(n);
    for (c, &start) in chunks.iter().enumerate() {
        let per = runs.iter().rev().find(|r| r.0 <= c + 1).map_or(1, |r| r.1);
        let mut at = start;
        for _ in 0..per {
            if offsets.len() == n {
                return Some(offsets);
            }
            let size = size(offsets.len())?;
            offsets.push(at);
            at += u64::from(size);
        }
    }
    Some(offsets)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each chapter's start and title, to compare.
    fn marks(chapters: Vec<Chapter>) -> Vec<(f64, String)> {
        chapters.into_iter().map(|c| (c.start, c.title)).collect()
    }

    /// A box of `kind` holding `body`.
    fn mp4box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend(kind);
        b.extend(body);
        b
    }

    fn be(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    fn file(boxes: &[Vec<u8>]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        std::fs::write(&path, boxes.concat()).unwrap();
        (dir, path)
    }

    #[test]
    fn nero_s_list_names_each_start() {
        let mut chpl = vec![1, 0, 0, 0, 0, 0, 0, 0, 2];
        for (start, title) in [(0u64, "The Whale"), (615_000_000, "The Camel")] {
            chpl.extend(start.to_be_bytes());
            chpl.push(title.len() as u8);
            chpl.extend(title.as_bytes());
        }
        let moov = mp4box(b"moov", &mp4box(b"udta", &mp4box(b"chpl", &chpl)));
        let (_d, path) = file(&[mp4box(b"ftyp", b"M4B "), moov]);
        assert_eq!(
            marks(read(&path)),
            [(0.0, "The Whale".into()), (61.5, "The Camel".into())]
        );
    }

    #[test]
    fn a_chapter_track_s_samples_are_its_titles() {
        // The titles first, in mdat, at 8 bytes in: a length, then the text.
        let titles: Vec<u8> = ["Whale", "Camel"]
            .iter()
            .flat_map(|t| [&(t.len() as u16).to_be_bytes()[..], t.as_bytes()].concat())
            .collect();
        let mdat = mp4box(b"mdat", &titles);
        let audio = mp4box(
            b"trak",
            &[
                mp4box(b"tkhd", &be(&[0, 0, 0, 1])),
                mp4box(b"tref", &mp4box(b"chap", &be(&[2]))),
            ]
            .concat(),
        );
        let stbl = [
            // Two samples, 30 s each at 1000 a second.
            mp4box(b"stts", &be(&[0, 1, 2, 30_000])),
            // One chunk of both, sized 7 each, at mdat's body.
            mp4box(b"stsc", &be(&[0, 1, 1, 2, 1])),
            mp4box(b"stsz", &be(&[0, 7, 2])),
            mp4box(b"stco", &be(&[0, 1, 8])),
        ]
        .concat();
        let minf = mp4box(b"minf", &mp4box(b"stbl", &stbl));
        let mdia = [mp4box(b"mdhd", &be(&[0, 0, 0, 1000, 60_000])), minf].concat();
        let text = mp4box(
            b"trak",
            &[mp4box(b"tkhd", &be(&[0, 0, 0, 2])), mp4box(b"mdia", &mdia)].concat(),
        );
        let moov = mp4box(b"moov", &[audio, text].concat());
        let (_d, path) = file(&[mdat, moov]);
        assert_eq!(
            marks(read(&path)),
            [(0.0, "Whale".into()), (30.0, "Camel".into())]
        );
    }

    /// An ID3 frame, its size as `version` writes it.
    fn frame(id: &[u8; 4], body: &[u8], version: u8) -> Vec<u8> {
        let n = body.len() as u32;
        let size = match version {
            4 => [n >> 21, n >> 14, n >> 7, n].map(|b| (b & 0x7f) as u8),
            _ => n.to_be_bytes(),
        };
        [&id[..], &size, &[0, 0], body].concat()
    }

    fn chap(name: &str, start_ms: u32, title: &[u8], version: u8) -> Vec<u8> {
        let tit2 = frame(b"TIT2", title, version);
        let times = be(&[start_ms, start_ms + 1000, u32::MAX, u32::MAX]);
        let body = [name.as_bytes(), &[0], &times, &tit2].concat();
        frame(b"CHAP", &body, version)
    }

    fn id3_file(version: u8, frames: &[Vec<u8>]) -> (tempfile::TempDir, std::path::PathBuf) {
        let body = frames.concat();
        let n = body.len() as u32;
        let size = [n >> 21, n >> 14, n >> 7, n].map(|b| (b & 0x7f) as u8);
        let tag = [&b"ID3"[..], &[version, 0, 0], &size, &body].concat();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.mp3");
        std::fs::write(&path, [tag, vec![0xff, 0xfb, 0, 0]].concat()).unwrap();
        (dir, path)
    }

    #[test]
    fn an_mp3_s_chap_frames_are_its_chapters_in_order_by_start() {
        for version in [3, 4] {
            let (_d, path) = id3_file(
                version,
                &[
                    frame(b"TIT2", b"\x03The Book", version),
                    chap("ch1", 61_500, b"\x03Two", version),
                    chap("ch0", 0, b"\x01\xff\xfeO\0n\0e\0", version),
                ],
            );
            let chapters = marks(read(&path));
            assert_eq!(
                chapters,
                [(0.0, "One".to_string()), (61.5, "Two".to_string())],
                "ID3v2.{version}, UTF-16 and UTF-8"
            );
        }
        let (_d, path) = id3_file(4, &[frame(b"TIT2", b"\x03No chapters", 4)]);
        assert!(read(&path).is_empty());
    }

    #[test]
    fn a_broken_file_is_read_no_further_than_it_holds() {
        // A run of four billion chapters is not a book's: read to the most, not to memory's end.
        let runs = starts(&be(&[0, 1, u32::MAX, 1])).unwrap();
        assert_eq!(runs.len(), CHAPTERS_MAX);
        // A box that says it is far past the end, which added to where it is would come round to
        // a box before it.
        let free = [&be(&[8])[..], b"free"].concat();
        let past = [&be(&[1])[..], b"free", &(u64::MAX - 7).to_be_bytes()].concat();
        let (_d, path) = file(&[mp4box(b"ftyp", b"M4B "), free, past]);
        assert!(read(&path).is_empty());
    }

    #[test]
    fn a_file_with_none_has_none() {
        let (_d, path) = file(&[mp4box(b"ftyp", b"M4A "), mp4box(b"moov", &[])]);
        assert!(read(&path).is_empty());
        assert!(read(Path::new("/no/such/book.m4b")).is_empty());
    }
}
