//! Album pictures. After a scan, each album without a current label gets one, a small PNG in
//! `Labels/Music`, from the front picture its first song carries or else a picture in its
//! folder (cover.jpg and the like). Albums with neither get a blank label, so they are not
//! looked at again until their songs change.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::library::tags;
use crate::library::{AlbumId, Library};

/// An album's picture, shrunk to fit the square the screen asked for.
pub struct Art {
    pub album: AlbumId,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// An album's label to make: from its first song, or its folder, or a blank.
pub struct Make {
    pub album: AlbumId,
    pub song: PathBuf,
    pub label: PathBuf,
}

/// Shrunk pictures, beside the cart labels.
const LABELS: &str = "Labels/Music";
/// Pictures players look for beside the songs, by name without the extension.
const FOLDER_PICTURES: [&str; 4] = ["cover", "folder", "front", "album"];

/// The album's label, named for the album, its artist and title, as players key their pictures.
pub fn label(root: &Path, lib: &Library, album: AlbumId) -> PathBuf {
    let album = &lib.albums[album];
    let key = format!("{}\0{}", lib.artists[album.artist].name, album.name).to_lowercase();
    root.join(LABELS)
        .join(format!("{:016x}.png", fnv(key.as_bytes())))
}

/// The albums whose label is missing, or older than their newest song.
pub fn missing(root: &Path, lib: &Library) -> Vec<AlbumId> {
    (0..lib.albums.len())
        .map(AlbumId)
        .filter(|&a| {
            let made = std::fs::metadata(label(root, lib, a))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            made.is_none_or(|made| made.as_secs() < lib.albums[a].changed)
        })
        .collect()
}

/// FNV-1a: a short name that stays the same from build to build.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The pass that files the missing labels, one album at a time.
pub struct Covers {
    total: usize,
    pending: VecDeque<AlbumId>,
    working: bool,
}

impl Covers {
    pub fn new(missing: Vec<AlbumId>) -> Covers {
        Covers {
            total: missing.len(),
            pending: missing.into(),
            working: false,
        }
    }

    /// Albums done and to do, while the pass runs.
    pub fn progress(&self) -> Option<(usize, usize)> {
        let left = self.pending.len() + usize::from(self.working);
        (left > 0).then(|| (self.total - left, self.total))
    }

    /// The next album's job, once the last one is done.
    pub fn next(&mut self, root: &Path, lib: &Library) -> Option<Make> {
        if self.working {
            return None;
        }
        let album = self.pending.pop_front()?;
        let song = lib.tracks[*lib.albums[album].tracks.first()?].path.clone();
        self.working = true;
        Some(Make {
            album,
            song,
            label: label(root, lib, album),
        })
    }

    /// The worker filed its album.
    pub fn made(&mut self) {
        self.working = false;
    }
}

/// Makes `job`'s label, shrunk to fit `px`: on the worker, as it reads the card.
pub fn make(job: &Make, px: u32) {
    let (song, label) = (&job.song, &job.label);
    let img = match picture(song) {
        Some(Ok(img)) => img.thumbnail(px, px).to_rgba8(),
        Some(Err(e)) => {
            eprintln!("slot: music: {}: {e}", song.display());
            image::RgbaImage::new(1, 1)
        }
        None => image::RgbaImage::new(1, 1),
    };
    if let Err(e) = file_label(&img, label) {
        eprintln!("slot: music: {}: {e}", label.display());
    }
}

/// `album`'s label to show, shrunk to fit `px`; none when it is missing or blank, the album
/// having no picture.
pub fn load(album: AlbumId, label: &Path, px: u32) -> Option<Art> {
    match image::open(label) {
        Ok(img) if img.width() <= 1 => None,
        Ok(img) => {
            let img = img.thumbnail(px, px).to_rgba8();
            let (w, h) = img.dimensions();
            Some(Art {
                album,
                w,
                h,
                rgba: img.into_raw(),
            })
        }
        Err(image::ImageError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("slot: music: {}: {e}", label.display());
            None
        }
    }
}

/// The picture in `song`, or else one beside it; none when there is neither.
fn picture(song: &Path) -> Option<image::ImageResult<image::DynamicImage>> {
    if let Some(bytes) = tags::picture(song) {
        return Some(image::load_from_memory(&bytes));
    }
    let entries = std::fs::read_dir(song.parent()?).ok()?;
    let beside = entries.flatten().map(|e| e.path()).find(|p| {
        let named = |part: Option<&std::ffi::OsStr>, any: &[&str]| {
            part.and_then(|s| s.to_str())
                .is_some_and(|s| any.iter().any(|a| a.eq_ignore_ascii_case(s)))
        };
        named(p.file_stem(), &FOLDER_PICTURES) && named(p.extension(), &["jpg", "jpeg", "png"])
    })?;
    Some(image::open(beside))
}

/// Written under a temporary name first, so a half-written picture is never read.
pub fn file_label(img: &image::RgbaImage, path: &Path) -> image::ImageResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = path.with_extension("png.part");
    img.save_with_format(&part, image::ImageFormat::Png)?;
    std::fs::rename(&part, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shrunk_picture_is_filed_whole_under_its_album_name() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Labels/Music/Aerosmith - Get a Grip.png");
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]));
        file_label(&img, &path).unwrap();
        let back = image::open(&path).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (3, 2));
        assert!(!path.with_extension("png.part").exists());
        assert!(
            load(AlbumId(0), &path, 2).is_some_and(|art| art.w == 2),
            "read back, shrunk"
        );
        assert!(
            load(AlbumId(0), &d.path().join("none.png"), 2).is_none(),
            "none yet: no picture"
        );
    }

    fn aged(path: &Path, secs_ago: u64) {
        let t = std::time::SystemTime::now() - std::time::Duration::from_secs(secs_ago);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn an_album_needs_a_label_when_it_has_none_or_its_songs_changed_since() {
        let d = tempfile::tempdir().unwrap();
        for f in ["A/Old/1 t.mp3", "A/Current/1 t.mp3", "A/None/1 t.mp3"] {
            let p = d.path().join("Music").join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
            aged(&p, 50);
        }
        let lib = Library::scan(d.path());
        let album = |name: &str| AlbumId(lib.albums.iter().position(|a| a.name == name).unwrap());
        let blank = image::RgbaImage::new(1, 1);
        for (name, age) in [("Old", 100), ("Current", 0)] {
            let path = label(d.path(), &lib, album(name));
            file_label(&blank, &path).unwrap();
            aged(&path, age);
        }
        let mut want = vec![album("Old"), album("None")];
        want.sort();
        assert_eq!(missing(d.path(), &lib), want);
    }

    #[test]
    fn a_label_is_named_for_its_album_and_made_from_a_picture_beside_the_songs() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("Music/A/B");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("01 t.mp3"), b"").unwrap();
        let picture = image::RgbaImage::from_fn(64, 32, |x, _| image::Rgba([x as u8, 0, 0, 255]));
        picture.save(folder.join("Cover.PNG")).unwrap();
        let lib = Library::scan(d.path());
        let path = label(d.path(), &lib, AlbumId(0));
        assert_eq!(
            path,
            label(d.path(), &lib, AlbumId(0)),
            "the same every time"
        );
        let mut covers = Covers::new(vec![AlbumId(0)]);
        assert_eq!(covers.progress(), Some((0, 1)));
        let job = covers.next(d.path(), &lib).unwrap();
        assert!(covers.next(d.path(), &lib).is_none(), "one at a time");
        make(&job, 16);
        covers.made();
        assert_eq!(covers.progress(), None);
        assert_eq!(image::open(&path).unwrap().width(), 16, "shrunk to fit");
    }
}
