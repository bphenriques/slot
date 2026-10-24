//! Finds which carts earn RetroAchievements offline, off the frame loop.
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const SETTLE: Duration = Duration::from_millis(500);

pub(crate) struct Seals {
    asks: Sender<()>,
    found: Receiver<HashSet<PathBuf>>,
    asked: Option<u64>,
    shown: bool,
}

impl Seals {
    pub fn spawn(root: PathBuf) -> Self {
        let (asks, inbox) = mpsc::channel::<()>();
        let (outbox, found) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-seals".into())
            .spawn(move || {
                let mut readiness = slot_achievements::Readiness::default();
                let mut first = true;
                while inbox.recv().is_ok() {
                    // Preparing a library writes a cache a second; scan once per burst.
                    if !first {
                        thread::sleep(SETTLE);
                    }
                    first = false;
                    while inbox.try_recv().is_ok() {}
                    if outbox.send(readiness.scan(&root)).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: seals: worker thread failed to start: {e}");
        }
        Seals {
            asks,
            found,
            asked: None,
            shown: false,
        }
    }

    /// Looks again whenever `generation`, the achievement cache's, moves. Returns the ready
    /// ROMs, and whether a change is news: the first answer only restores what was there.
    pub fn poll(&mut self, generation: u64) -> Option<(HashSet<PathBuf>, bool)> {
        if self.asked != Some(generation) {
            self.asked = Some(generation);
            let _ = self.asks.send(());
        }
        let ready = self.found.try_iter().last()?;
        let news = self.shown;
        self.shown = true;
        Some((ready, news))
    }
}
