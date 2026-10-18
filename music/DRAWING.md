# Drawing only what changed

The music screens are drawn whole 60 times a second, changed or not. Lit Now Playing, paused,
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
(`music_faces`, by `Spec`), as ghostty keeps rows. And unseen means nothing: the lid closed draws
nothing (`App::resting`).

## The plan

One rule for every view: **a frame is drawn when what it would draw differs, on the panel, from
what was drawn last.** The music screen already ends each frame as one list of draws
(`slot_gfx::Draw`: rectangles and textures, `PartialEq`), built from the `Screen`. Comparing that
list, rather than the `Screen`, catches everything that shows, with no code for any one view or
animation:

- a view's change, as its `Screen` changes;
- motion: a slide, a fade, a marquee, a hold's ring, which move the draws;
- what the glue adds: slot's clock and battery in the header.

Two things make the comparison honest:

- **On the panel's pixels.** Places and sizes are compared as whole pixels, and alpha and colour
  in 255 steps, as the panel shows them. A bar growing by a tenth of a pixel a frame is no
  change, and is drawn when it reaches the next pixel.
- **A face rewritten is a change.** A texture written again under the same id (a line whose text
  changed) draws the same list, so the faces' upload says it changed.

The steps:

1. **Skip the frames that change nothing.** The glue keeps the last list drawn and wants a frame
   only when the new one differs, or a face was written. In slot's loop, one hook beside the
   lid's: no frame wanted, no render and no swap. The panel keeps showing the last frame, as it
   does under the lid. Only on the music screens: anything of slot's over them (its quick
   menu, a toast) draws every frame, as now. The list is still built each frame (~0.1 ms, under 1% of a core), and is
   the price of not tracking anything.
2. **Measure.** The performance log gains frames drawn beside frames run.
3. **Then, measured, perhaps sleep.** The loop still wakes 60 times a second to read the buttons
   and build the list, ~1% as the dark shows. Going further means ghostty's way: sleeping until
   a button or the next wake, so motion would have to say when it next moves. That is upstream's
   loop and more to keep right. Done only if the numbers say it is worth it.

## Expected

- Lit and still (paused, a list unmoved): from ~10% to ~2%, near the dark's.
- Lit and playing: a frame when the time's second or the bar's pixel changes, and while a long
  title scrolls.
- While something moves (a slide, a scroll, a held key): as now, 60 frames a second.

## Tests

- **The decision:** a pure `same_on_panel(last, now)` beside the kit's draws: a sub-pixel growth
  is the same, a pixel's is not, an alpha step under 1/255 is the same.
- **The glue:** `music_faces` says a face rewritten is a change; a still Now Playing, paused, wants
  no frame after the first (slot lib test, as `every_music_legend_fits_across_the_screen`).
- **On the device:** frames drawn a second fall to what changed.

## Not done

- Drawing only part of the screen (damage rectangles): ghostty doesn't either. At 640×480 a full
  compose costs little once it is drawn rarely, and partial drawing would reach into upstream's
  compositor.
- Comparing `Screen`s with each animation saying when it next moves: closer to ghostty, but every
  new animation would have to remember to say so, and one that forgot would freeze. Comparing
  what is drawn cannot miss one. It is step 3's way, if step 3 is ever needed.
