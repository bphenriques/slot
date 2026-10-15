use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::library::Library;
use crate::muse::Command;
use crate::Art;

/// Where Muse writes an album's embedded picture, before it is shrunk.
const EXTRACTED: &str = "/tmp/slot-covers";
/// Shrunk pictures, named for their album, beside the cart labels; one can be added by hand.
const LABELS: &str = "Labels/Music";

pub fn label(root: &Path, lib: &Library, album: usize) -> PathBuf {
    let album = &lib.albums[album];
    let artist = &lib.artists[album.artist].name;
    root.join(LABELS)
        .join(format!("{artist} - {}.png", album.name))
}

/// A blank label is far smaller than any picture, so its size identifies it.
const BLANK_MAX: u64 = 200;

/// The albums to ask Muse about: those with no label, and those whose blank label is older
/// than their first track (re-tagged since). Labels with a picture are kept.
pub fn missing(root: &Path, lib: &Library) -> Vec<usize> {
    (0..lib.albums.len())
        .filter(|&a| match std::fs::metadata(label(root, lib, a)) {
            Err(_) => true,
            Ok(m) if m.len() <= BLANK_MAX => retagged(lib, a, &m),
            Ok(_) => false,
        })
        .collect()
}

fn retagged(lib: &Library, album: usize, blank: &std::fs::Metadata) -> bool {
    let first = lib.albums[album]
        .tracks
        .first()
        .map(|&t| &lib.tracks[t].path);
    let changed = first.and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
    match (changed, blank.modified()) {
        (Some(track), Ok(blank)) => track > blank,
        _ => false,
    }
}

/// Gives each album a picture after a scan, one at a time: Muse extracts it from the first
/// track and the worker shrinks it into `Labels/Music`. Albums without one get a blank label.
pub struct Covers {
    total: usize,
    pending: VecDeque<usize>,
    asked: Option<(usize, PathBuf)>,
    shrinking: bool,
}

impl Covers {
    pub fn new(missing: Vec<usize>) -> Covers {
        Covers {
            total: missing.len(),
            pending: missing.into(),
            asked: None,
            shrinking: false,
        }
    }

    /// Albums done and to do, while the pass runs.
    pub fn progress(&self) -> Option<(usize, usize)> {
        let working = usize::from(self.asked.is_some() || self.shrinking);
        let left = self.pending.len() + working;
        (left > 0).then(|| (self.total - left, self.total))
    }

    /// The next album to ask Muse about, once the last one is done.
    pub fn next(&mut self, lib: &Library) -> Option<Command> {
        if self.asked.is_some() || self.shrinking {
            return None;
        }
        let album = self.pending.pop_front()?;
        let path = lib.tracks[*lib.albums[album].tracks.first()?].path.clone();
        let base = Path::new(EXTRACTED).join(album.to_string());
        self.asked = Some((album, base.clone()));
        Some(Command::Cover { path, base })
    }

    /// Muse's answer: the picture to shrink and file, or a blank.
    pub fn answered(
        &mut self,
        root: &Path,
        lib: &Library,
        base: &Path,
        file: Option<PathBuf>,
    ) -> Option<Job> {
        let (album, _) = self.asked.take_if(|(_, b)| b == base)?;
        self.shrinking = true;
        Some(Job::Shrink {
            album,
            file,
            label: label(root, lib, album),
        })
    }

    /// The worker filed its album.
    pub fn shrunk(&mut self) {
        self.shrinking = false;
    }

    /// Muse went away before answering: ask again later.
    pub fn lost(&mut self) {
        if let Some((album, _)) = self.asked.take() {
            self.pending.push_front(album);
        }
    }
}

pub enum Job {
    /// Shrink Muse's picture into the album's label, or file a blank.
    Shrink {
        album: usize,
        file: Option<PathBuf>,
        label: PathBuf,
    },
    /// Read a label for the screen; one added by hand may be any size.
    Load { album: usize, label: PathBuf },
}

pub enum Done {
    Shrunk(usize),
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
        Job::Shrink { album, file, label } => {
            let img = match &file {
                Some(file) => image::open(file).map(|img| img.thumbnail(px, px).to_rgba8()),
                None => Ok(image::RgbaImage::new(1, 1)),
            };
            match img {
                Ok(img) => {
                    if let Err(e) = file_label(&img, &label) {
                        eprintln!("slot: music: {}: {e}", label.display());
                    }
                }
                Err(e) => eprintln!("slot: music: {}: {e}", label.display()),
            }
            if let Some(file) = file {
                let _ = std::fs::remove_file(file);
            }
            Some(Done::Shrunk(album))
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
    fn a_blank_label_older_than_its_music_is_asked_about_again_and_a_picture_never_is() {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "A/Blank old/1 t.mp3",
            "A/Blank new/1 t.mp3",
            "A/Picture/1 t.mp3",
            "A/None/1 t.mp3",
        ] {
            let p = d.path().join("Music").join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        let lib = Library::scan(d.path());
        let album = |name: &str| lib.albums.iter().position(|a| a.name == name).unwrap();
        let blank = image::RgbaImage::new(1, 1);
        let picture =
            image::RgbaImage::from_fn(64, 64, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
        for (name, img, label_age) in [
            ("Blank old", &blank, 100),
            ("Blank new", &blank, 0),
            ("Picture", &picture, 100),
        ] {
            let path = label(d.path(), &lib, album(name));
            file_label(img, &path).unwrap();
            aged(&path, label_age);
        }
        for t in &lib.tracks {
            aged(&t.path, 50);
        }
        let mut want = vec![album("Blank old"), album("None")];
        want.sort();
        assert_eq!(missing(d.path(), &lib), want);
    }

    #[test]
    fn progress_counts_the_albums_left_and_ends_when_none_are() {
        assert_eq!(Covers::new(Vec::new()).progress(), None);
        let mut c = Covers::new(vec![4, 7, 9]);
        assert_eq!(c.progress(), Some((0, 3)));
        // 4 is done, and 7 is taken off the list to be asked about.
        c.pending.pop_front();
        c.pending.pop_front();
        c.asked = Some((7, PathBuf::from("/tmp/x")));
        assert_eq!(
            c.progress(),
            Some((1, 3)),
            "the one asked about is not done yet"
        );
        c.asked = None;
        c.pending.clear();
        assert_eq!(c.progress(), None);
    }
}
