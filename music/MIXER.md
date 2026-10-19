# Phase 2: one owner of the sound

Parked until the player is stable on the device. This is what we know, what we would change
and what we would check first.

## Why

BaseOS has no mixer: one program at a time opens the codec. slot has two openers, its own
sink (menu sounds, games) and slot-player (music), so today they take turns. The turn-taking
is what these exist for, and all of it goes in phase 2:

- `Music::owns_audio` / `Ring::set_yielded`: slot's sink steps aside while music plays.
- The busy-device wait in `slot-player/src/alsa.rs`: music waits for slot's sink to let go.
- `slot/src/audio/line_out.rs`: music's volume set on the codec (`lineout volume`, 0–31,
  1.5 dB a step, numbers from plorpOS, marked provisional there), restored after; and
  `Status::sounding` / `Player::audible`, which keep the codec at music's level until the
  device has faded out, so a pause's tail is not heard at slot's level.
- The menu sounds are silent while music plays, as the device is music's.

## What BaseOS gives, and why the mixer is not BaseOS's

From [pvaibhav/BaseOS](https://github.com/pvaibhav/BaseOS) (docs 01, 05 and 06):

- The vendor's ALSA, `/etc/asound.conf` and kernel as they come; BaseOS adds a speaker amplifier
  module, `alsactl` across sleep, BlueALSA and an optional startup sound (up to 30 s at boot;
  the device may be busy meanwhile, which slot already rides out).
- ALSA's own mixer, `dmix`, needs SysV IPC, and plorpOS says this kernel has none; BaseOS
  never rebuilds the kernel. To check on the device: `ls /proc/sysvipc`.
- A system mixer would be a daemon (PipeWire or our own): against BaseOS's aim (small,
  neutral to the frontend), latency for games, battery while idle, and it would serve programs
  slot does not run. Not worth proposing.
- Bluetooth playback is untested by BaseOS ("daemon startup" only).

Every sound is in slot's own process: menu sounds, games (the `slot-emu` thread) and the music
player. So one owner inside slot is enough.

## The shape

The music player stops opening ALSA and writes into slot's sink instead: a second `Output`
for slot-player, beside `alsa.rs`, given to it by the glue (`slot::app::music`).

- **The rate.** slot opens its sink at 32,768 Hz, the GBA's, even with no game; menu sounds
  are resampled to it. Music at that rate loses everything above ~16 kHz, so while music plays
  the sink reopens at the music's rate (`Ring::reopen`): the song's own, as slot-player now opens
  the device (44.1 kHz for the whole card), or 48 kHz where the device will not take it; and
  goes back as a game starts. Menu sounds are rendered at whatever the sink runs at. Music and
  games never sound together (a game pauses the music, a settled decision), so nothing is
  resampled for mixing.
- **The mix.** Music pushes into the ring as a game core does (`push_blocking`, the ring
  pacing it); menu sounds keep `Ring::mix`, laid over what is queued. slot's software volume
  (`audio/volume.rs`) applies to music too, so the codec keeps one level and `line_out.rs`
  goes.
- **The place.** `Output::delay` is what the ring holds plus the device's delay, so Status
  still says what is heard.
- **The de-click.** A pause now rewinds the device and fades 10 ms out (`alsa.rs`). With the
  ring, the same is done to what the ring holds: drop it, keeping 10 ms faded. Fade-in after a
  start or a seek stays in the player.
- **Releasing.** slot's sink lets the device go 3 s after the last sound (`RELEASE_AFTER`),
  for sleep and for others; music's own release goes with its device.

Touches to slot's own files: the sink reopening at a rate it is asked, and the glue handing the
ring to the player. The rest is in our crates.

## Ready, and still to do

Ready in slot-player (2026-10-18):

- **The rate is the engine's choice, the device's answer.** The engine picks a file's own rate,
  the rate playing for a file gone on into (gapless, crossfade), or 48 kHz once the device has
  refused the file's; `Output::set_rate` asks, before anything is written at it, and may say no.
  Nothing above slot-player knows rates. Phase 2's output answers `set_rate` by reopening slot's
  sink at that rate (`Ring::reopen`), and says no where the sink's device does.
- **One owner.** The writer thread owns all that plays (files, output, counts, state) and
  publishes `Status` as one value; the engine thread only opens files. The ring output is one
  more `Output` the writer owns, so nothing about threads changes for it.
- **Whether still heard** (`Status::sounding`) comes from the output being let go, whatever the
  output is.

Still to do, at phase 2:

- `set_rate` answered across threads: the sink opens its device on its own thread, so the ring
  output waits for its answer (a reply on a channel), rare and off the frame loop.
- Dither, the 10 ms fade-in and the fade-out on a cut are in `alsa.rs`; the ring output needs
  the same, so they move to one place both outputs use. Not before: one user so far.
- slot's volume on music (`audio/volume.rs`), in place of the codec's.

## Gone after

`owns_audio`, `set_yielded`, `audible`/`sounding` as a reason for the codec, `line_out.rs`, the
busy-device retry, and the reason the menu sounds go quiet under music.

## Kept out until asked

- Music during a game. plorpOS's way, the game silent while music plays, needs no mixer and
  almost no code (see below); mixed under the music, it needs resampling one side and a
  decision on levels (ducking).
- Bluetooth headphones: frontends would need a device name that follows the headset; BaseOS
  has not tested playback. The one place BaseOS and slot would need to work together.

## Before starting, on the device

- `ls /proc/sysvipc`, to confirm dmix is out.
- `cat /etc/asound.conf`, to see what `default` is (routing, locked controls) before the sink
  relies on it.
- The perf log: `sound` says what the codec really runs at while music plays, and what the
  ring's thread costs with music through it.

## Worth sending to BaseOS

A short note for frontends, as a docs issue or PR: which PCM to open, that it is exclusive,
the controls the default PCM locks, and the volume control to use and its steps. Cheap, and
every frontend gains from it.

## How plorpOS does it

From [thwonp/plorpOS](https://github.com/thwonp/plorpOS) (`src/muse/muse.c`, `pcm.c`, `dec.c`,
`docs/guide.md`), read 2026-10-18:

- The same constraint, the same answer: "the H700's codec is handed over, not shared". Muse
  and the game's audio take turns on `default`; Muse waits up to a second for the device, and
  closes it 500 ms after it stops playing.
- Music during a game: allowed, and the game is silent while it plays ("only the game's own
  sound"); pausing the music brings the game back within a tenth of a second. SELECT in the
  in-game menu opens Muse.
- Muse decodes to a fixed 48 kHz because the Brick's dmix runs at 48 kHz and Allwinner's ALSA
  resampler (`awrate`) is of unknown quality; on the H700 it opens `default` at 48 kHz with
  ALSA's resampling off. We open at the song's own rate instead, with ALSA's resampling off too,
  and fall back to our own resampler at 48 kHz if the device refuses.
- Muse's whole cost was ~1% of a core: FFmpeg's decoder and its 64-tap resampler. Ours is the
  pure-Rust decoder (symphonia) plus rubato; the perf log says how far apart.

What slot already has toward the same: while music holds the device, slot's sink keeps draining
the game's ring on a timer and drops it (`audio/alsa.rs`, `yielded`), so a game would keep its
pace and be silent. What stands in the way is only our rule that a game pauses the music, and
a way to reach the music from a game (slot's in-game menu has no music row). Both are product
calls before phase 2; with phase 2's single owner, the game could be mixed under the music
instead of silenced.
