use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use slot_gfx::{Compositor, TexId};
use slot_store::Cart;
use slot_ui::{cart_face_with, cart_interior, shows_interior, CartFace};

use crate::app::{App, FaceWant};
use crate::label_cache::label_art;

const WORKERS: usize = 2;

type Key = (usize, usize);

/// What a face was drawn from. It is placed only while its cart still matches.
#[derive(PartialEq)]
struct Drawn {
    rom: PathBuf,
    label: Option<PathBuf>,
    sealed: bool,
}

impl Drawn {
    fn of(cart: &Cart) -> Self {
        Drawn {
            rom: cart.rom.clone(),
            label: cart.label.clone(),
            sealed: cart.sealed,
        }
    }
}

/// A cart's face, what it was drawn from, and, for a clear GBA cart, the game-coloured inside
/// it is drawn over.
type Built = (Key, Drawn, CartFace, Option<CartFace>);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<(Key, Cart)>,
    closed: bool,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

pub struct CartFaces {
    queue: Shared,
    built: Receiver<Built>,
    pending: HashSet<Key>,
    asked: Vec<FaceWant>,
    free: Vec<TexId>,
}

impl CartFaces {
    pub fn spawn(root: PathBuf) -> Self {
        let queue: Shared = Arc::default();
        let (outbox, built) = mpsc::channel();
        for n in 0..WORKERS {
            let (queue, outbox, root) = (queue.clone(), outbox.clone(), root.clone());
            let spawned = thread::Builder::new()
                .name(format!("slot-carts-{n}"))
                .spawn(move || work(&queue, &outbox, &root));
            if let Err(e) = spawned {
                eprintln!("slot: cart faces: worker thread failed to start: {e}");
            }
        }
        CartFaces {
            queue,
            built,
            pending: HashSet::new(),
            asked: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        self.free.extend(app.shed_faces());
        while let Ok((key, drawn, face, interior)) = self.built.try_recv() {
            self.place(app, compositor, key, drawn, face, interior);
        }
        let wants = app.face_wants();
        if wants != self.asked {
            self.requeue(app, &wants);
            self.asked = wants;
        }
        let mut waiting: Vec<Key> = self
            .asked
            .iter()
            .filter(|w| w.on_screen)
            .map(|w| (w.shelf, w.index))
            .collect();
        while !waiting.is_empty() {
            let Ok((key, drawn, face, interior)) = self.built.recv() else {
                return;
            };
            if self.place(app, compositor, key, drawn, face, interior) {
                waiting.retain(|k| *k != key);
            }
        }
    }

    fn requeue(&mut self, app: &App, wants: &[FaceWant]) {
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
        for (key, _) in queue.jobs.drain(..) {
            self.pending.remove(&key);
        }
        for w in wants {
            let key = (w.shelf, w.index);
            if self.pending.contains(&key) {
                continue;
            }
            if let Some(cart) = app.face_cart(w.shelf, w.index) {
                queue.jobs.push_back((key, cart.clone()));
                self.pending.insert(key);
            }
        }
        wake.notify_all();
    }

    /// A label can arrive, or a seal come or go, while a face is drawn. Its cart is then drawn
    /// again as it is now, first in line, and the key stays pending.
    fn redraw_if_stale(&mut self, app: &App, key: Key, drawn: &Drawn) -> bool {
        let Some(cart) = app
            .face_cart(key.0, key.1)
            .filter(|c| Drawn::of(c) != *drawn)
        else {
            return false;
        };
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
        queue.jobs.push_front((key, cart.clone()));
        wake.notify_all();
        true
    }

    fn texture(&mut self, compositor: &mut Compositor, face: &CartFace) -> TexId {
        match self.free.pop() {
            Some(tex) => {
                compositor.update_texture(tex, face.w, face.h, &face.rgba);
                tex
            }
            None => compositor.create_texture(face.w, face.h, &face.rgba),
        }
    }

    /// False when the face is out of date, and is dropped. See [`Self::redraw_if_stale`].
    fn place(
        &mut self,
        app: &mut App,
        compositor: &mut Compositor,
        key: Key,
        drawn: Drawn,
        face: CartFace,
        interior: Option<CartFace>,
    ) -> bool {
        if self.redraw_if_stale(app, key, &drawn) {
            return false;
        }
        self.pending.remove(&key);
        self.asked.retain(|w| (w.shelf, w.index) != key);
        let tex = self.texture(compositor, &face);
        self.free.extend(app.set_cart_face(key.0, key.1, tex));
        if let Some(interior) = interior {
            let tex = self.texture(compositor, &interior);
            self.free.extend(app.set_cart_interior(key.0, key.1, tex));
        }
        true
    }
}

impl Drop for CartFaces {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        wake.notify_all();
    }
}

fn work(queue: &Shared, outbox: &Sender<Built>, root: &Path) {
    let (lock, wake) = &**queue;
    loop {
        let (key, cart) = {
            let mut q = lock.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if q.closed {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    break job;
                }
                q = wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let face = cart_face_with(&cart, label_art(root, &cart));
        let interior = shows_interior(&cart).then(|| cart_interior(&cart));
        if outbox
            .send((key, Drawn::of(&cart), face, interior))
            .is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_store::Platform;

    fn cart(stem: &str) -> Cart {
        Cart {
            platform: Platform::Gba,
            stem: stem.into(),
            rom: format!("/x/{stem}.gba").into(),
            label: None,
            title: String::new(),
            code: String::new(),
            shell: None,
            sealed: false,
        }
    }

    fn faces(key: Key) -> CartFaces {
        let (_, built) = mpsc::channel();
        CartFaces {
            queue: Arc::default(),
            built,
            pending: HashSet::from([key]),
            asked: Vec::new(),
            free: Vec::new(),
        }
    }

    fn queued(faces: &CartFaces) -> Vec<(Key, Cart)> {
        faces.queue.0.lock().unwrap().jobs.iter().cloned().collect()
    }

    #[test]
    fn a_face_drawn_before_the_cart_was_sealed_is_drawn_again() {
        let mut app = App::new(vec![cart("Sealed")]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces(key);
        let before = Drawn::of(app.face_cart(key.0, key.1).unwrap());
        app.seal(&HashSet::from([PathBuf::from("/x/Sealed.gba")]), false);
        assert!(faces.redraw_if_stale(&app, key, &before));
        let jobs = queued(&faces);
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].1.sealed, "the cart is drawn again as it is now");
        assert!(faces.pending.contains(&key), "and not asked for twice");
        let now = Drawn::of(app.face_cart(key.0, key.1).unwrap());
        assert!(!faces.redraw_if_stale(&app, key, &now));
    }

    #[test]
    fn a_face_drawn_before_its_label_arrived_is_drawn_again() {
        let mut app = App::new(vec![cart("Labelled")]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces(key);
        app.seal(&HashSet::from([PathBuf::from("/x/Labelled.gba")]), false);
        let before = Drawn::of(app.face_cart(key.0, key.1).unwrap());
        app.attach_label(Path::new("/x/Labelled.gba"), "/l/Labelled.png".into());
        assert!(faces.redraw_if_stale(&app, key, &before));
        assert_eq!(
            queued(&faces)[0].1.label,
            Some(PathBuf::from("/l/Labelled.png"))
        );
    }
}
