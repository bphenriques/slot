//! The worker: one thread, at low priority, for what reads or writes the card slowly and would
//! hold up a frame: album pictures, a song's lyrics and a book's chapters. Each job carries the
//! library's generation, so what comes back from before a rescan is dropped.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::books::chapters::{self, Chapter};
use crate::library::covers::{self, Art, Make};
use crate::library::{AlbumId, TrackId};
use crate::lyrics::{self, Lyrics};

pub(crate) enum Job {
    /// Make an album's label.
    Make(Make),
    /// Read an album's label for the screen.
    Load { album: AlbumId, label: PathBuf },
    /// Read an album's thumbnail, for a list's row.
    Thumb { album: AlbumId, thumb: PathBuf },
    /// Read a book's part's chapters, an MP4's own or an MP3's.
    Chapters { track: TrackId, path: PathBuf },
    /// Read a song's lyrics, from an `.lrc` beside it or its tags.
    Lyrics { track: TrackId, path: PathBuf },
}

pub(crate) enum Done {
    Made(AlbumId),
    /// An album's picture, or none: it has no picture, or no label yet.
    Loaded(AlbumId, Option<Art>),
    /// An album's thumbnail, or none.
    Thumb(AlbumId, Option<Art>),
    Chapters(TrackId, Vec<Chapter>),
    /// A song's lyrics, or none: it has none.
    Lyrics(TrackId, Option<Lyrics>),
}

pub(crate) struct Worker {
    jobs: Sender<(u32, Job)>,
    done: Receiver<(u32, Done)>,
}

impl Worker {
    /// `px` is the square pictures are shrunk to fit, `thumb_px` a list row's; `frames` is the frame loop's thread, woken
    /// as each job is done, so what it read shows at once rather than at the loop's next wake.
    pub fn spawn(px: u32, thumb_px: u32, frames: std::thread::Thread) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<(u32, Job)>();
        let (done_tx, done) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-music-work".into())
            .spawn(move || {
                crate::background();
                let mut queue = Queue::default();
                loop {
                    // Waits only with nothing to do; takes in what came meanwhile either way.
                    if queue.is_empty() {
                        match job_rx.recv() {
                            Ok(job) => queue.push(job),
                            Err(_) => return,
                        }
                    }
                    job_rx.try_iter().for_each(|job| queue.push(job));
                    let Some((generation, job)) = queue.next() else {
                        continue;
                    };
                    let done = guarded(job, |job| work(job, px, thumb_px));
                    if done_tx.send((generation, done)).is_err() {
                        return;
                    }
                    frames.unpark();
                }
            });
        if let Err(e) = spawned {
            log::warn!("{e}");
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

/// The jobs waiting: what a screen waits on first, the labels a scan left to make after, so a
/// song's lyrics or a book's chapters never wait behind a picture being made.
#[derive(Default)]
struct Queue {
    reads: VecDeque<(u32, Job)>,
    makes: VecDeque<(u32, Job)>,
}

impl Queue {
    fn push(&mut self, job: (u32, Job)) {
        match job.1 {
            Job::Make(_) => self.makes.push_back(job),
            _ => self.reads.push_back(job),
        }
    }

    fn next(&mut self) -> Option<(u32, Job)> {
        self.reads.pop_front().or_else(|| self.makes.pop_front())
    }

    fn is_empty(&self) -> bool {
        self.reads.is_empty() && self.makes.is_empty()
    }
}

/// `job` done by `work`; or, should a file make it panic, as though it found nothing, so one
/// broken file costs only its own picture, lyrics or chapters, and the worker goes on.
fn guarded(job: Job, work: impl FnOnce(Job) -> Done) -> Done {
    let nothing = match &job {
        Job::Make(make) => Done::Made(make.album),
        Job::Load { album, .. } => Done::Loaded(*album, None),
        Job::Thumb { album, .. } => Done::Thumb(*album, None),
        Job::Chapters { track, .. } => Done::Chapters(*track, Vec::new()),
        Job::Lyrics { track, .. } => Done::Lyrics(*track, None),
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(job))).unwrap_or(nothing)
}

fn work(job: Job, px: u32, thumb_px: u32) -> Done {
    match job {
        Job::Make(make) => {
            covers::make(&make, px, thumb_px);
            Done::Made(make.album)
        }
        Job::Load { album, label } => Done::Loaded(album, covers::load(album, &label, px)),
        Job::Thumb { album, thumb } => Done::Thumb(album, covers::load(album, &thumb, thumb_px)),
        Job::Chapters { track, path } => Done::Chapters(track, chapters::read(&path)),
        Job::Lyrics { track, path } => Done::Lyrics(track, lyrics::read(&path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::AlbumId;

    #[test]
    fn a_job_done_wakes_the_frame_loop_resting_rather_than_wait_for_its_next_wake() {
        let worker = Worker::spawn(16, 8, std::thread::current());
        let lyrics = Job::Lyrics {
            track: TrackId(1),
            path: PathBuf::from("/no/such/song.mp3"),
        };
        let began = std::time::Instant::now();
        worker.send(0, lyrics);
        while worker.done().next().is_none() {
            std::thread::park_timeout(std::time::Duration::from_secs(5));
            assert!(began.elapsed() < std::time::Duration::from_secs(2), "woken");
        }
    }

    #[test]
    fn a_job_that_panics_finds_nothing_rather_than_end_the_worker() {
        let lyrics = Job::Lyrics {
            track: TrackId(1),
            path: PathBuf::from("a.mp3"),
        };
        let done = guarded(lyrics, |_| panic!("a broken file"));
        assert!(matches!(done, Done::Lyrics(TrackId(1), None)));
    }

    #[test]
    fn what_a_screen_waits_on_goes_before_the_labels_to_make() {
        let make = |album| {
            let (song, label) = (PathBuf::from("a.mp3"), PathBuf::from("a.png"));
            let thumb = PathBuf::from("a.thumb.png");
            (
                0,
                Job::Make(Make {
                    album,
                    song,
                    label,
                    thumb,
                    changed: 0,
                }),
            )
        };
        let mut queue = Queue::default();
        queue.push(make(AlbumId(0)));
        queue.push(make(AlbumId(1)));
        let path = PathBuf::from("b.mp3");
        queue.push((
            0,
            Job::Chapters {
                track: TrackId(3),
                path,
            },
        ));
        assert!(matches!(queue.next(), Some((_, Job::Chapters { .. }))));
        assert!(matches!(queue.next(), Some((_, Job::Make(m))) if m.album == AlbumId(0)));
        assert!(matches!(queue.next(), Some((_, Job::Make(_)))));
        assert!(queue.next().is_none() && queue.is_empty());
    }
}
