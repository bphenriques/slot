# Phase 2: one owner of the sound — held

Re-assessed 2026-10-19 and held: today's turn-taking works, and a mixer costs more than it
gives until a product need asks for one.

## Today

BaseOS has no mixer: one program at a time opens the codec. slot's own sink (menu sounds, games)
and slot-player (music) take turns: the sink steps aside while music plays
(`Music::owns_audio`, `Ring::set_yielded`), music waits up to a second for the device, the codec
follows slot's volume through music's fades (`slot/src/audio/line_out.rs`), and menu sounds are
silent under music. About 260 lines, measured stable on the device, nothing while idle.

ALSA's `dmix` needs SysV IPC, which this kernel lacks (plorpOS; check with `ls /proc/sysvipc`),
and a system daemon would cost latency and battery. Every sound is in slot's process, so one
owner inside slot is the shape, if ever.

## What a mixer would cost

- **Upstream footprint**: slot's sink reopening at music's rate, its ring handed to the player,
  music on slot's software volume: hooks in slot's audio, where merges are hardest.
- **CPU**: one more copy and another thread waking each period; a less direct write already
  measured worse on this chip (the mapped-buffer trial, DEVELOPMENT.md).
- **Quality**: slot's ring is 16-bit and its volume scales after the cut, losing bits at low
  volume; the codec's own volume does not.
- **Clicks**: the pause fade, the amplifier kept on silence, and a rate change between games
  (32,768 Hz) and music (44.1 kHz), which reopens the device, all rebuilt on the ring.

## What would bring it back

- Music during a game. Silent game, as plorpOS has it: no mixer, but an in-game way to reach the
  music and dropping the rule that a game pauses it. Mixed: the full mixer.
- More than one app making sound, as an OS-like shell would have. Then the mixer belongs in
  slot's audio, as the platform's, not in the music crate; slot-player's `Output` is already
  the seam a ring output would plug into.

## Worth sending to BaseOS

A short note for frontends: which PCM to open, that it is exclusive, the controls the default
PCM locks, and the volume control to use and its steps.
