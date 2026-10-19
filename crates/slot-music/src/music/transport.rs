//! The transport as the state runs it: fades in and out, the sleep timer, play and pause, the
//! sound told the player for what plays, and Recently Played as songs are heard.

use super::{Effect, Music};
use crate::integrations::scrobbler;
use crate::integrations::Listen;
use crate::playback::fade::{Fade, Fading, SleepTimer, FADE, SLEEP_FADE};
use crate::playback::recent::Recent;
use crate::store::Kept;
use slot_player::{Command, Leveling, PlayState};

/// The sound last told the player for what plays.
#[derive(Copy, Clone, PartialEq)]
pub(super) struct Sound {
    crossfade: u32,
    skip_silence: bool,
    boost: u32,
    level: Leveling,
    mono: bool,
    eq: usize,
}

/// The transport's own: a fade running, the sleep timer, and the sound last told the player.
#[derive(Default)]
pub(crate) struct Transport {
    pub(crate) fade: Option<Fade>,
    pub(crate) sleep: SleepTimer,
    pub(super) sound: Option<Sound>,
}

impl Music {
    /// Acts on held keys, runs the sleep timer and ends fades.
    pub(super) fn tick(&mut self) {
        let now = self.now.at;
        if let Some(press) = self.buttons.keys.due(now) {
            self.act(press);
        }
        let ends = self
            .end_left()
            .is_some_and(|left| self.transport.sleep.end_due(left));
        if self.transport.sleep.due(now) || ends {
            self.fall_asleep();
        }
        if let Some(fade) = self.transport.fade.filter(|f| f.done(now)) {
            self.transport.fade = None;
            if fade.is_out() {
                self.pause_now();
            }
            // Asleep, the listener likely missed a while before the timer ran out.
            if fade.fading == Fading::Asleep && self.book().is_some() {
                let back = self.player.seek(-crate::books::SLEPT_BACK);
                self.send(back);
            }
        }
    }

    /// The sleep timer ran out: the music fades out to a pause, or, silent already, pauses.
    fn fall_asleep(&mut self) {
        if self.player.owns_audio() {
            let fade = Fade::new(self.volume_scale(), Fading::Asleep, SLEEP_FADE, self.now.at);
            self.transport.fade = Some(fade);
        } else {
            self.pause_now()
        }
    }

    /// Pauses at once, or plays, fading in. A press during a fade out turns it back; the sleep
    /// timer's then starts again.
    pub(super) fn toggle(&mut self) -> Option<Command> {
        // A saved queue still being read back plays once it is.
        if self.player.queue().waiting {
            self.scan.play_when_read ^= true;
            return None;
        }
        if let Some(fade) = self.transport.fade.filter(Fade::is_out) {
            if fade.fading == Fading::Asleep {
                self.transport.sleep.again(self.now.at);
            }
            self.transport.fade = Some(Fade::new(
                self.volume_scale(),
                Fading::In,
                FADE,
                self.now.at,
            ));
            return None;
        }
        // A book picks up a little back.
        let rewind = self.resume_book();
        self.send(rewind);
        let command = self.player.toggle(&self.library);
        if self.player.owns_audio() {
            self.transport.fade = Some(Fade::new(0.0, Fading::In, FADE, self.now.at));
        }
        command
    }

    /// What slot's volume is scaled by: the fade's level, 1 otherwise. A pause keeps the level
    /// it had: the player fades its own last moment out, and the codec cut to nothing under it
    /// would click.
    pub fn volume_scale(&self) -> f32 {
        self.transport.fade.map_or(1.0, |f| f.level(self.now.at))
    }

    /// For a game starting or the screen going off: at once, without a fade.
    pub fn pause(&mut self) {
        self.stamp();
        self.pause_now();
        self.run();
    }

    pub(super) fn pause_now(&mut self) {
        self.transport.fade = None;
        let command = self.player.pause();
        self.send(command);
        self.keep_queue();
    }

    /// Tells the player the sound for what plays, each part as it changes, all of it to a player
    /// new: a book is never crossfaded, nor a song into the next on its album, played in order,
    /// as Plexamp keeps an album gapless; only a book skips silence and is boosted.
    pub(super) fn tell_sound(&mut self) {
        let along = self.player.follows_on_album(&self.library);
        let s = &self.settings;
        let level = s.level(self.player.shuffled());
        let sound = if self.book().is_some() {
            Sound {
                crossfade: 0,
                skip_silence: s.skip_silence,
                boost: s.boost,
                level,
                mono: s.mono,
                eq: s.eq,
            }
        } else {
            Sound {
                crossfade: if along { 0 } else { s.crossfade },
                skip_silence: false,
                boost: 0,
                level,
                mono: s.mono,
                eq: s.eq,
            }
        };
        let was = self.transport.sound.replace(sound);
        let boost = 10f32.powf(sound.boost as f32 / 20.0);
        let told = [
            (
                changed(was, sound, |s| s.level),
                Command::Level(sound.level),
            ),
            (
                changed(was, sound, |s| s.crossfade),
                Command::Crossfade(f64::from(sound.crossfade)),
            ),
            (
                changed(was, sound, |s| s.skip_silence),
                Command::SkipSilence(sound.skip_silence),
            ),
            (changed(was, sound, |s| s.boost), Command::Boost(boost)),
            (changed(was, sound, |s| s.mono), Command::Mono(sound.mono)),
        ];
        let told = told.into_iter().filter(|(new, _)| *new).map(|(_, c)| c);
        let eq = changed(was, sound, |s| s.eq)
            .then(|| Command::Equalizer(self.equalizer.bands(sound.eq)));
        self.out.extend(told.chain(eq).map(Effect::Player));
    }

    /// Puts the song playing first in Recently Played once it has played a while.
    pub(super) fn count_played(&mut self) {
        let Some(t) = self.player.current().filter(|_| self.book().is_none()) else {
            return;
        };
        let (at, len) = self.player.time();
        let playing = self.player.state() == PlayState::Playing;
        if self.recent.hear(t, at, len, playing) {
            let text = self.recent.encode(self.card());
            self.out.push(Effect::Write(Recent::FILE.into(), text));
        }
        // Listened to, as Last.fm counts it: told to the scrobbler log, when it is kept.
        if self.settings.scrobbler && self.scrobbler.hear(t, at, len, playing) {
            let track = &self.library.tracks[t];
            let listen = Listen {
                artist: track.artist.clone(),
                album: self.library.album_of(t).name.clone(),
                title: track.title.clone(),
                number: track.number.clone(),
                secs: len,
                began: self.now.wall.saturating_sub(at as u64),
            };
            let line = scrobbler::line(&listen);
            self.out.push(Effect::Append {
                file: scrobbler::FILE,
                header: scrobbler::HEADER,
                line,
            });
        }
    }
}

/// Whether `part` of the sound differs from what it `was`, or there was none: a player new.
fn changed<T: PartialEq>(was: Option<Sound>, now: Sound, part: impl Fn(Sound) -> T) -> bool {
    was.is_none_or(|was| part(was) != part(now))
}
