# Developing this fork

The rules for working here, for people and agents alike, are in `AGENTS.md`; this is the longer
guide.

This fork is upstream slot (pvaibhav/slot, itself BrandonKowalski/slot) plus a music and
audiobook player. Keep it that way: new behaviour lives in new crates and modules, upstream
files change only at a few one-line hooks, so `git merge upstream/...` stays cheap.

## Layout

| Where | What |
|---|---|
| `crates/slot-music` | The player's behaviour, no drawing, tested without a device: `music/` (`Music`: the state and its shell, by concern: `act`, `loading`, `art`, `books`, `lyrics`, `pocket`, `worker`), `library/` (scan, tags, cache, pictures, playlists), `playback/` (player, queue, fades, Recently Played), `books/`, `browse/` (views, keys, rows), `screens/` (each screen as data), `lyrics.rs`, `settings.rs`, `text.rs`. |
| `crates/slot-player` | Plays files: the `Command`/`Event` words and `Handle`, the face every player has. slot's own engine (symphonia decode, rubato resample, SoundTouch speed, ReplayGain, mono, gapless, ALSA): an engine thread opens files, a writer thread owns all that plays and publishes `Status` whole (see AGENTS.md, Threads). |
| `crates/slot-player/soundtouch` | SoundTouch 2.3.2 sources (LGPL-2.1), built by `build.rs` with `cc`. |
| `crates/slot-music/src/store` | Everything kept on the card: one writer, each file's format (`Kept`). |
| `crates/slot-ui/src/music` | The kit that draws: faces, lists, Now Playing, the text view, `tween.rs` (every move), `theme.rs` (every colour). |
| `crates/slot/src/music_faces`, `crates/slot/src/app/music.rs` | The frontend glue: textures, animation (`motion.rs`), input routing. |
| `music/` | `CONTROLS.md` (every button, keep it current), `music.txt.example`. |

Upstream files touched: `task footprint` lists them, as changed from upstream's. Keep those
diffs small. `crates/slot/src/audio/line_out.rs` is ours, new beside upstream's sound: the codec's
level following music's fades.

## Set up

- NixOS/Nix: `nix-shell` gives `task`, `rustup`, rsync and the rest (`shell.nix`); the toolchain
  is pinned in `rust-toolchain.toml`. Run cargo through it: `nix-shell --run "cargo ..."`.
- Device builds are cross-compiled on an x86_64 host with `cargo zigbuild` for the device's
  glibc (2.31, bullseye's), zig compiling SoundTouch's C++ and linking its runtime in: about a
  minute, where compiling in an emulated arm64 container took seven or more. On an arm64 Linux box
  they build natively. The container (`rust:1-bullseye`, Docker with aarch64 binfmt) remains for
  the cores and for running the device binary once to lay out the card. Nothing here uses bindgen
  or links ALSA at build time; ALSA is loaded when it runs.

## Build, test, lint

```sh
nix-shell --run "cargo test -p slot-music -p slot-ui -p slot-input -p slot-player"
nix-shell --run "cargo test -p slot --no-default-features --features slot/device --lib"
nix-shell --run "cargo clippy -p slot -p slot-services --no-default-features --features slot/device -- -D warnings"
nix-shell --run "cargo clippy -p slot-music -p slot-ui -p slot-input -p slot-player --tests -- -D warnings"
nix-shell --run "cargo fmt -p slot-music -p slot-ui -p slot -p slot-player"
git checkout -- crates/slot/tests/prefetch.rs crates/slot/tests/runahead.rs   # fmt touches these upstream files; keep them as upstream has them
```

All of it passes before anything goes on a card. On an x86 laptop, slot's own `input`, `bios`,
`core`, `core_options` and emulator timing tests fail regardless of changes; the music lib tests
above do not. `every_music_legend_fits_across_the_screen` (slot lib) fails when a hint is too
long.

## Deploy

```sh
nix-shell --run "task dist:device"                        # builds dist-device/
C=/run/media/$USER/<card>                                  # SD2, the frontend card
mount | grep "$C"; cat /sys/block/mmcblk0/ro               # must be rw and 0 (the lock switch)
nix-shell --run "task sync:card CARD=$C"                   # adds and replaces; keeps games, saves, config
cmp dist-device/System/slot "$C/System/slot" && sync       # verify, then unmount
```

To measure on the device: Music › Settings › Debug › Performance Log On, use the player a while
(screen on and off, playing and paused, a book), then read `Logs/music-perf.log` on the card:
one line every 10 s with frames a second, the music frame, screen and draw times (average and
longest, in microseconds), each thread's share of one core, the player's own (decoding, mixing,
writing, and of the writing, ALSA's library's), the processor's clock (its average over the line
where the kernel keeps that, and its governor: a share of a core is more work at a slower
clock), memory, and what was shown, with the song playing.

Measured so (2026-10-18, one song, the lid closed, a share of one core): decoding a packet at a
time rather than rodio's sample at a time took decoding from 2.95% to 2.08%. FFmpeg 6.1's
`mp3float`, Muse's, with its NEON, plugged in as symphonia's decoder (the same sound to within
2.5e-6 on ten kinds of MP3), took 2.31% on the device, 32% faster on x86 but 11% slower on the
A53, so symphonia stays. Of the writing (1.0%), ALSA's library took 0.1% and the dither most of
the rest: one draw a sample for its two uniform values instead of two draws took it to 0.6%, the
player's thread from 3.4% to 3.0%. The clock was at its lowest, 480 MHz, nearly throughout:
3% there is about 1% at its 1512 MHz, where Muse's ~1% was likely measured.

Over adb instead: `task deploy:device`. Logs: `Logs/slot.log` on the card (or
`task log:device`). Player settings live in `Config/music.txt`.

## Conventions

- Simple, idiomatic, succinct, composable; YAGNI; familiar UX (look at how common players do it).
- Every user-visible word in `crates/slot-music/src/text.rs`; every colour and level in
  `crates/slot-ui/src/music/theme.rs`. No literals elsewhere.
- Logic emits values (`Outcome`, `Notice`, `Legend`); `slot-ui` and the frontend only draw.
- Buttons: a tap does one thing, a hold another, shown as "Tap (Held)"; hints never change
  while held (the corner ring shows the hold). START menus hold only what no button does.
  Update `music/CONTROLS.md` with any key change.
- Comments and docs in the existing voice: plain sentences saying what something is for.
- Tests for behaviour (`crates/slot-music/src/tests/`, a module an area; the music as a whole in `src/music/tests.rs`, `crates/slot-player/tests/player.rs`,
  which plays WAVs made in the test into a `Capture` output). Timing tests keep wide margins.
- Performance: the device is a battery handheld (H700, Cortex-A53). Nothing polls while idle;
  work only while playing; measure before and after (see Status).
- Editing with scripts: no Python on the host; with perl, `\|` in a `|`-delimited regex is an
  alternation (use `~`), `'` comes out literally from heredoc variables (fix with
  `sed -i "s/\'/'/g"`), and check a temp file with `test -s` before copying it over.

## Git

- Commit only features the owner has tested end to end on the device; never per change.
- Author email `4727729+bphenriques@users.noreply.github.com`; no other address ever.
- Never push or tag without an explicit go. The version is `v1.7.0-bphenriques.1`.
- `stash@{0}` is old music work from before the v1.7.0 merge; drop it once happy.

## Device quirks

What the RG SP (H700, BaseOS) does that another device may not, each met where it is, named
there, and found from the logs on the card rather than guessed. None is a build setting: the
device build is plain release, and these are the hardware's and BaseOS's.

| Quirk | Seen as | Met in |
|---|---|---|
| The speaker's amplifier switches off with the sound device, heard as a click however quietly the sound ends. | A click on every pause, the fade-out logged clean each time. | `slot-player/src/alsa.rs`, `KEPT_PAUSED`: a pause keeps the device on silence 3 s. |
| BaseOS has no mixer: one opener of the codec at a time. | slot's own sound busy while music plays, and the other way round. | Music owns the device while it plays and lets it go after (`MIXER.md` for the plan beyond). |
| The device takes 44.1 kHz as well as 48 kHz. | "sound at 44100 Hz" in `slot.log`. | `Output::set_rate`: a file plays at its own rate, resampled only when refused. |
| The place the device says it is at moves in steps, behind what it is reading. | A click when all it held was taken back; and the place said going back a little after each write, the bar redrawn twice as often as it moves. | `alsa.rs`, `taken_back`: 50 ms is left to play out; `engine.rs`, `Writer::write`: what is said heard never goes back. |
| The codec's level is `lineout volume` on `hw:0`, 0 to 31 at 1.5 dB a step, as plorpOS found. | Music played straight to the codec, at slot's volume. | `slot/src/audio/line_out.rs`, followed through music's fades. |
| Two shoulders squeezed together come in as one moment, in either order. | The button trace with the performance log on. | `music/controller.rs`: any two of them are the pocket combo. |

A device after this one gets its own row, and its own answer where the row says.

## Status and what is left

Everything is committed up to `acbac22`. Measured on the device (`Logs/music-perf.log`): playing
a 44.1 kHz file the player is ~4% of a core; with the lid closed, ~7% playing and ~2% paused;
Now Playing lit, ~22% (the frame ~9%, the GPU driver ~8%). Next, in order:

1. Draw only what changed: the lit screens are most of the cost, and are drawn whole each frame.
2. Phase 2, the mixer (`music/MIXER.md`).
3. Release notes and licences: SoundTouch's is in `licenses/`; list symphonia, rubato
   and lofty with the other third-party crates.

A card from before Muse was gone keeps an unused `System/muse/` and `System/muse.sh`, safe to
delete.

Known gaps: chapters only from MP4 books and MP3s' ID3 `CHAP` frames; Bluetooth output is whatever ALSA's
`default` is (no device choice yet); the old keys in the card's `music.txt` (`skip`, `rows`,
`fade`, `animations`) are ignored; its `player` key is ignored now. Playlist editing is out of scope (see
`music/CONTROLS.md`).

Ideas, if wanted: Bluetooth output choice; equalizer presets; crossfade only between albums. Each fits as a stage in
`slot-player/src/track.rs` or `engine.rs`, behind a setting on the Sound page.

A screensaver, from Now Playing with SELECT held (its tap opens the queue; the hold is free): blank
first, which is the lid closed without closing it (the backlight off and nothing drawn, as
`App::resting` already does), so the keys under the lid act as they do there; a visualisation later,
which would cost the drawing the blank saves, so worth it only measured.
