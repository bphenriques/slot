//! Album pictures. After a scan, each album without a current label gets one, a small PNG in
//! `Labels/Music`, from the front picture its first song carries or else a picture in its
//! folder (cover.jpg and the like). Albums with neither get a blank label, so they are not
//! looked at again until their songs change.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::UNIX_EPOCH;

use crate::library::Library;
use crate::{tags, Art};

/// Shrunk pictures, beside the cart labels.
const LABELS: &str = "Labels/Music";
/// Pictures players look for beside the songs, by name without the extension.
const FOLDER_PICTURES: [&str; 4] = ["cover", "folder", "front", "album"];

/// The album's label, named for the album, its artist and title, as players key their pictures.
pub fn label(root: &Path, lib: &Library, album: usize) -> PathBuf {
    let album = &lib.albums[album];
    let key = format!("{}\0{}", lib.artists[album.artist].name, album.name).to_lowercase();
    root.join(LABELS)
        .join(format!("{:016x}.png", fnv(key.as_bytes())))
}

/// The albums whose label is missing, or older than their newest song.
pub fn missing(root: &Path, lib: &Library) -> Vec<usize> {
    (0..lib.albums.len())
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
    pending: VecDeque<usize>,
    working: bool,
}

impl Covers {
    pub fn new(missing: Vec<usize>) -> Covers {
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
    pub fn next(&mut self, root: &Path, lib: &Library) -> Option<Job> {
        if self.working {
            return None;
        }
        let album = self.pending.pop_front()?;
        let song = lib.tracks[*lib.albums[album].tracks.first()?].path.clone();
        self.working = true;
        Some(Job::Make {
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

pub enum Job {
    /// Make the album's label from its first song or folder, or a blank.
    Make {
        album: usize,
        song: PathBuf,
        label: PathBuf,
    },
    /// Read a label for the screen.
    Load { album: usize, label: PathBuf },
}

pub enum Done {
    Made(usize),
    Loaded(Art),
}

/// The pictures' thread, at low priority. Jobs carry the library generation, so results from
/// before a rescan are dropped.
pub struct Worker {
    jobs: Sender<(u32, Job)>,
    done: Receiver<(u32, Done)>,
}

impl Worker {
    /// `px` is the square pictures are shrunk to fit.
    pub fn spawn(px: u32) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<(u32, Job)>();
        let (done_tx, done) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-music-art".into())
            .spawn(move || {
                crate::background();
                for (generation, job) in job_rx {
                    if let Some(result) = work(job, px) {
                        if done_tx.send((generation, result)).is_err() {
                            return;
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        Worker { jobs, done }
    }

    pub fn send(&self, generation: u32, job: Job) {
        let _ = self.jobs.send((generation, job));
    }

    pub fn done(&self) -> impl Iterator<Item = (u32, Done)> + '_ {
        self.done.try_iter()
    }
}

fn work(job: Job, px: u32) -> Option<Done> {
    match job {
        Job::Make { album, song, label } => {
            let img = match picture(&song) {
                Some(Ok(img)) => img.thumbnail(px, px).to_rgba8(),
                Some(Err(e)) => {
                    eprintln!("slot: music: {}: {e}", song.display());
                    image::RgbaImage::new(1, 1)
                }
                None => image::RgbaImage::new(1, 1),
            };
            if let Err(e) = file_label(&img, &label) {
                eprintln!("slot: music: {}: {e}", label.display());
            }
            Some(Done::Made(album))
        }
        Job::Load { album, label } => match image::open(&label) {
            // A blank label: the album has no picture.
            Ok(img) if img.width() <= 1 => None,
            Ok(img) => {
                let img = img.thumbnail(px, px).to_rgba8();
                let (w, h) = img.dimensions();
                Some(Done::Loaded(Art {
                    album,
                    w,
                    h,
                    rgba: img.into_raw(),
                }))
            }
            Err(e) => {
                eprintln!("slot: music: {}: {e}", label.display());
                None
            }
        },
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
        let album = |name: &str| lib.albums.iter().position(|a| a.name == name).unwrap();
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
        let path = label(d.path(), &lib, 0);
        assert_eq!(path, label(d.path(), &lib, 0), "the same every time");
        let mut covers = Covers::new(vec![0]);
        assert_eq!(covers.progress(), Some((0, 1)));
        let job = covers.next(d.path(), &lib).unwrap();
        assert!(covers.next(d.path(), &lib).is_none(), "one at a time");
        assert!(matches!(work(job, 16), Some(Done::Made(0))));
        covers.made();
        assert_eq!(covers.progress(), None);
        assert_eq!(image::open(&path).unwrap().width(), 16, "shrunk to fit");
    }
}
