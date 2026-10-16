//! A book's chapters read from its file, as players show them before it plays: an MP4's
//! (an .m4b's) QuickTime chapter track, or else Nero's chapter list. Muse tells them again once
//! it plays, for every kind of file.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The file's metadata box is read whole; one larger than this is not a book's.
const MOOV_MAX: u64 = 16 << 20;

/// Each chapter's start, in seconds, and its title; none when the file names none.
pub fn read(path: &Path) -> Vec<(f64, String)> {
    let mut file = File::open(path).ok();
    let moov = file.as_mut().and_then(moov).unwrap_or_default();
    let track = file.and_then(|mut f| track(&mut f, &moov));
    track.or_else(|| chpl(&moov)).unwrap_or_default()
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
            0 => (file.metadata().ok()?.len() - at, 8),
            size => (size, 8),
        };
        if moov && size <= MOOV_MAX {
            let mut moov = vec![0; size.checked_sub(skip)? as usize];
            file.read_exact(&mut moov).ok()?;
            return Some(moov);
        }
        at += size.max(8);
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

/// When each sample starts, in the track's time scale, from the runs of their durations.
fn starts(stts: &[u8]) -> Option<Vec<u64>> {
    let mut starts = Vec::new();
    let mut t = 0;
    for run in 0..u32_at(stts, 4)? as usize {
        let (n, d) = (u32_at(stts, 8 + run * 8)?, u32_at(stts, 12 + run * 8)?);
        for _ in 0..n {
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
            read(&path),
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
        assert_eq!(read(&path), [(0.0, "Whale".into()), (30.0, "Camel".into())]);
    }

    #[test]
    fn a_file_with_none_has_none() {
        let (_d, path) = file(&[mp4box(b"ftyp", b"M4A "), mp4box(b"moov", &[])]);
        assert!(read(&path).is_empty());
        assert!(read(Path::new("/no/such/book.m4b")).is_empty());
    }
}
