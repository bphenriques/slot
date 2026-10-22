# Drawing only what changed

Before this, the music screens were drawn whole 60 times a second, changed or not. Lit Now Playing, paused,
costs ~5% of a core on the main thread and ~4.6% in the GPU driver (`mali-cmar-backe`); the same
screen dark, nothing drawn, ~1.1% and 0%. Our own share of a frame is small: building the
`Screen` ~10 µs, its faces ~25 µs, the draw list ~90 µs, about 0.8% of a core at 60 Hz. The cost
is in drawing at all: the compose and the swap. So the plan is to not draw, not to draw less.

## What ghostty does

Ghostty's renderer (`src/renderer/Thread.zig`, `generic.zig`) is the model:

- **Drawn when woken, never on a clock.** A change wakes the renderer, and wakes that come
  together are one frame. With nothing changing, nothing runs.
- **A frame is drawn only when it needs to be.** It draws when the cells were rebuilt, the size
  changed, or an animation is due (`needs_redraw`); otherwise it skips the frame, GPU and all.
  When it does draw, it draws everything: no partial GPU updates. The saving is the frames not
  drawn.
- **Animations ask for their next wake.** `animationWake()` says when the next moving frame is
  due, and one timer is set for it. The cursor blinks on its own 600 ms timer. Nothing asks for
  a frame "in case".
- **What is built is kept.** Rows are rebuilt only when marked dirty; a font or config change
  marks everything.
- **Unseen, nothing.** While hidden, frames and animations stop, and becoming visible draws once
  to catch up.

We already have two of these. The faces are kept, built again only when their line changes
(`slot-music-view`, by `Spec`), as ghostty keeps rows. And unseen means nothing: the lid closed draws
nothing (`App::resting`).

## How it is done

Ghostty's pattern: **a frame is drawn when something changed, or something of it is due to
move.** Before each frame, the device loop asks the app (`App::holds_still`), at the hook it
already had for the closed lid; held still, nothing is built, drawn or swapped, and the panel
keeps the last frame. Everything else is in our own files.

- **What the music screen shows** (`app::music::Shown`), compared with what it showed when last
  drawn: the `Screen` slot-music builds, the album picture and colours the faces are drawn from,
  slot's own draws for the frame (anything of its over the music, as the volume), and the compositor's settings (brightness fading in on
  wake, the shader). A change to any of them is a frame.
- **When it next moves of itself** (the glue's `wake`, ghostty's `animationWake`), told with each
  frame drawn: the next frame while a slide, glide or fade is on its way (`Motion::moving`);
  else when the soonest scrolling line takes its next step (`marquee_next`, worked out from the
  marquee, never late by a step); else never, until something changes. All motion is in
  `motion.rs` and the marquee, as AGENTS.md has it, so the list is closed.

The first frame back on the music screen is always drawn, so what was up before is never left;
the rest of slot draws every frame as it always has, and so does the host.

What makes it pay is that nothing moves by less than can be seen:

- **The `Screen` says only what is seen.** The time is in seconds, and the bar's place in
  thousandths (`screens::fraction`), finer than any bar is wide. A playing Now Playing changes
  49 times in 10 s, not every frame.
- **The glue moves in whole steps.** Marquees scroll 2 px a step on one 40 ms beat for every
  line (`marquee_start`), so two long lines cost the frames of one, 25 a second while they move
  and none while they rest; the ring fills in 40 steps, the bar to the pixel, and a slide holds
  still once it has arrived.

Slot's own files gain two lines in `app.rs` (where the last drawn is kept) and a changed one in
the lid's hook.

## Measured

- **Locally, exactly:** `work_budget_…` in `slot-music/src/music/tests.rs`, on the test's clock:
  frames worth drawing in 10 s. Playing, 49 (was 500); paused, 0; a list left alone, 0. They are
  held as budgets, and `task perf:ab REV=<rev>` prints them for a revision beside the working
  tree. Counts, not times: the same on any machine. `holds_still` is tested on an `App`, and the
  marquee's next step against the marquee itself, millisecond by millisecond.
- **On the device:** the performance log says frames run and drawn a second (`60 fps, 1 drawn`),
  beside each thread's share of a core.

## Expected on the device

- Lit and still (paused, a list unmoved): from ~10% to ~2%, near the dark's.
- Lit and playing: a frame when the time's second or the bar's thousandth changes; with long
  lines, 25 a second while they scroll, none while they rest. Measured before the beat: 55 a
  second, two marquees stepping on frames of their own; after it, still twice what changed (12
  a second for about 6 changes), the loop drawing one frame more after each, as it had for the
  lid. The lid's one frame is now `App::holds_still`'s, and a change is one frame.
- While something moves (a slide, a glide, a held key): as before, 60 frames a second.

## Waking only when needed

Held still, the loop still woke 60 times a second, to read the buttons, run the music's frame
and ask whether to draw: ~0.8% of a core with the lid closed, most of what was left once frames
were not drawn. Now it rests until the next thing due, a button or the lid
(`App::next_frame`, `DeviceInput::rest`), as ghostty sleeps until a message or its next
deadline:

- **The next thing due** is the soonest of the glue's next step (a scrolling line's, `wake`) and
  the music's next change (`Music::due_in`): on a screen that follows the song (Now Playing, a
  book's, the lyrics), when it next shows it moved on, worked out beside each screen from what
  it shows (`steps_in`: the time's next second, the bar's next step, a book's share and time
  left, its chapter's end, the next line sung, the lyrics following again, the sleep timer's
  countdown), a millisecond past the step so that it has surely come. While the motor buzzes or
  sound fades, it is the next frame; on any other screen, or the lid closed, nothing.
- **It is never late**: `resting_until_the_next_change_is_never_late_…` plays a song, the
  countdown, the lyrics and a book at 1.5 times a millisecond at a time, resting as the loop
  does, and fails if a screen changes while it rests; none of its wakes finds nothing new.
- **A button wakes it at once**: the input threads wake the loop as they queue what they read.
  A button held is timed frame by frame, slot's (a volume held) as much as the music's, so the
  loop does not rest while one is down.
- **The lid is read every 150 ms**, as before, so a rest lasts at most that. What changes of
  itself unseen by the above (a notice going, the player going on into the next song in the
  dark) is late by at most as much.

So the lid closed, a list, or a still screen wakes about 7 times a second instead of 60, and
Now Playing as often as it shows something new. That is two of upstream's files, its loop and
its input, by a few lines each.

## Not done

- Drawing only part of the screen (damage rectangles): ghostty doesn't either. At 640×480 a full
  compose costs little once it is drawn rarely, and partial drawing would reach into upstream's
  compositor.
