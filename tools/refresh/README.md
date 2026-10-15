# RG SP refresh and run-ahead comparison

This harness runs the production Slot renderer, audio, and emulation worker on an
ADB-connected RG SP with the Slot card layout. The product also supports the
verified RG34XX SP v1 panel and the source-verified RG34XX panel, requiring exact
model/panel pairs and DT clock/timings. See [hardware evidence](HARDWARE.md).

Slot supports four core/system combinations: mGBA on GBA, GB, and GBC; gpSP on
GBA. The scene list covers each, plus a second GBA game on both cores. Run-ahead
is supported only for GBA/mGBA and uses one speculative frame on the same core,
followed by rollback. It preserves the committed game timeline.

## Build and run

Build on Linux AArch64, with the dependencies used by the device build:

```sh
cargo build -p slot --profile device --no-default-features --features device --target-dir target-device
rustc --edition=2021 -O -C strip=symbols tools/refresh/probe.rs -o target-device/refresh-probe
python3 -B tools/refresh/make_response_rom.py /tmp/slot-response.gba
python3 -B tools/refresh/profile.py \
  --serial ac001089c89588720d2 \
  --binary target-device/device/slot \
  --probe target-device/refresh-probe \
  --output /tmp/slot-refresh-results \
  --seconds 12 --warmup 5 --repeats 5 \
  --latency-rom /tmp/slot-response.gba --latency-count 60
```

The ROM generator needs Clang with the `armv4t-none-eabi` target. The default ROM
uses the core's HLE boot and contains no Nintendo logo. The optional `--header`
argument copies the logo from a caller-supplied ROM; do not distribute that file.

Use `--gpsp path/to/gpsp_libretro.so` to test a different core in the temporary
card roots. This never replaces the installed core. Its SHA-256 is recorded;
resuming with a different core is rejected while gpSP results are present.

Eject the current cartridge before starting. The harness pauses the original
Slot process, verifies it is stopped, and keeps the services daemon running. Each
test has a fresh temporary card root with copied settings, save RAM and resume
state. ROMs, cores and BIOS are read-only symlinks; user saves are never written.
Compare refresh modes within each core. Existing mGBA and gpSP resume states can
represent different scenes, so their absolute CPU numbers are not a controlled
comparison of core efficiency. Games without resume states follow their cold
boot/title sequence; this is representative coverage, not a guarantee for every
game or the most demanding scene in each game.
Every mode starts a new Slot process. The previous game is ejected and the test
process exits before the next process applies its refresh preference. Finally,
the original physical refresh mode is restored and init restarts the original
menu. Restarting closes its old evdev queues, which contain the injected test
keys; resuming a stopped frontend would replay those keys into the user's menu.
Ordinary exceptions and Ctrl-C use the same cleanup. Abrupt host/device loss
still requires manual recovery or reboot.

Supported panels default to 120 Hz, applied before game launch and
reapplied at resume. There is no user-visible refresh option. Run-Ahead remains
an independent Gameplay setting, off by default, for GBA on mGBA.
Unsupported hardware keeps its existing refresh behavior. The internal
`high_refresh=0` setting remains available for diagnostics and this harness.
Legacy configurations with no `high_refresh` key adopt the new default and
retain their existing run-ahead preference.

## What is measured

`probe sample PID gpsp|gba|gb SECONDS` reads the installed core's actual committed
frame counter through `/proc/PID/mem`. It resolves ELF symbols and validates the
mGBA accessor instructions rather than assuming an address in a particular
binary. Sampling does not stop or trace the target. The host fits frame count
against elapsed time; frontend presents are not used as proof of game speed.
Run-ahead can expose a transient speculative frame, so its slope has small
sampling noise. Unsupported or stripped core layouts fail explicitly.

At 100 ms intervals the probe also reads process CPU ticks, shared-policy CPU
frequency, SoC temperature, and LCD IRQ 293. CPU usage is summed across Slot's
threads: 100% means one fully busy CPU, on a four-CPU device. Average frequency
is a sample mean; this kernel does not expose frequency residency counters.
LCD IRQ slope must match the requested mode within 3 Hz, otherwise the run fails.
Base OS's `ref_fps` field is stale and is not used.

The performance mode order rotates between repetitions. Games without a resume
state get at least 12 seconds of warmup because cold-boot silence release/reopen
can interrupt ALSA drainage around nine seconds at either refresh rate. Audio fault deltas
come from the periodic log counters immediately before and after measurement;
their boundaries have approximately five-second resolution. Startup totals are
retained in the raw logs. CPU clock and temperature are battery proxies, not a
measurement of display power or battery life. Short interleaved runs are not
independent thermal-equilibrium measurements.

## Input response

`response.s` reads A once per GBA frame and displays the **previous** frame's
button state as red/green. This deliberate frame of game delay lets one-frame
run-ahead demonstrate a benefit. The host disables shader/color correction and
rewind for this controlled ROM, leaving the same frontend and renderer active.

`SLOT_TRACE_RESPONSE=FILE` records the selected source image's first pixel after
its EGL swap returns. The probe injects alternating A-down/up events into evdev
at varied phases, and matches the first changed game-image row. Blended first
responses count. White/black compositor clears do not count. Each trial has a
250 ms timeout; failures make the run fail instead of silently disappearing.
Latency trials rotate mode order and repeat too.

The reported interval is software event injection to changed-image swap return.
It excludes physical button debounce and does not measure LCD light output,
scanout position, or panel response time. The result is specific to this ROM's
one-frame delay. Real games can benefit differently from run-ahead. A faster
panel shortens the display wait, while normal input/emulation deadlines remain
at the core's native rate; 120 Hz does not halve the entire input path.

Raw per-run CSV/log/display files, binary hashes and settings snapshots accompany
`results.json`. A stopped run can continue with `--resume`; it rejects changed
binaries or measurement settings and skips completed cases. Use a fresh output
directory for a new comparison. Generate the checked tables with:

```sh
python3 -B tools/refresh/summarize.py /tmp/slot-refresh-results --output /tmp/slot-refresh-results/summary.md
```

Do not compare results from runs with failed isolation or mode
verification. The rejected smoke runs are deliberately excluded from the report.
