# RG SP 120 Hz and run-ahead results — 2026-10-15

120 Hz preserves native game speed across every core/system combination supported
by Slot, with a small CPU cost in the sampled scenes. It reduces the controlled
software response time by about 8.5 ms. At the user's request, the verified RG SP
panel now defaults to 120 Hz with no refresh menu option. Run-Ahead remains an
independent Gameplay setting and defaults to off.

## Coverage and method

The device is an Allwinner H700 RG SP, Base OS 1.4.0, exact target `rgsp` and
panel `rg34xxsp_v1`. Production capability detection checks the exact model/panel
pair, geometry, timings and clock before allowing any register writes.

All 60 planned runs completed: 54 performance runs and six latency runs. The
completed GBA comparisons have five runs per mode, the second gpSP game has
three, and GB/GBC and latency have two. Remaining repetitions were reduced at
the user's request. Modes rotate in order. Each performance window is 12 seconds,
after five seconds of warmup for resumed games or at least 12 seconds for cold
boots. Each mode launches a fresh process; refresh never changes during a game.

The probe reads actual committed core frame counters, rather than counting
presentations. Process CPU includes all Slot threads; 100% is one busy CPU out
of four. Shared-policy CPU frequency and SoC temperature are sampled every 100 ms
under `schedutil`, with limits 480–1,512 MHz. Frequency is a sample mean, not a
residency counter. Short interleaved temperatures are not equilibrium tests.
These proxies do not measure LCD power or establish a battery-life percentage.
Additional RG34XX SP live checks and RG34XX source evidence are recorded in
[the hardware report](HARDWARE.md); they are separate from this resource matrix.

Test scenes cover GBA/mGBA, GBA/gpSP, GB/mGBA and GBC/mGBA. Advance Wars uses
the existing per-core resume states; Mario Tennis: Power Tour follows its cold
boot/title sequence. GB uses Tetris, and GBC uses Mario Tennis. Different cores
can have different resume scenes, so compare modes within a core, rather than
treating these as a controlled mGBA-versus-gpSP efficiency ranking. Coverage is
representative, not every ROM or every demanding gameplay scene.

## Game speed and resource use

Values below are medians across runs. The [generated tables](results/rgsp-2026-10-15/summary.md)
include every mode, ranges, paired differences and diagnostics.

| Scene/core | Runs per side | 60 Hz game FPS | 120 Hz game FPS | Paired CPU change, percentage points | Paired frequency change | Paired temperature change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Advance Wars / mGBA | 5 | 59.741 | 59.734 | +2.42 | −2.6 MHz | +0.08°C |
| Advance Wars / gpSP | 5 | 59.724 | 59.727 | +2.25 | −8.1 MHz | +0.35°C |
| Power Tour / mGBA | 5 | 59.721 | 59.722 | +4.33 | −12.1 MHz | +0.44°C |
| Power Tour / gpSP | 3 | 59.716 | 59.730 | +2.25 | +2.0 MHz | +0.48°C |
| Tetris GB / mGBA | 2 | 59.705 | 59.723 | +2.33 | +19.7 MHz | +0.04°C |
| Mario Tennis GBC / mGBA | 2 | 59.729 | 59.714 | +1.62 | +33.8 MHz | +0.23°C |

Native speed is approximately 59.7275 FPS. All normal-mode samples were within
0.06% of nominal, including gpSP with ample CPU headroom. No measured normal-mode
audio drop/starvation deltas or timing warnings occurred. LCD IRQ 293 measured
59.50–59.58 Hz at the original setting and 120.08–120.17 Hz at the new setting.
The kernel's `ref_fps:59` field remains stale.

Frequency differences change sign across repetitions and overlap the observed
variation, so these data do not establish a consistent frequency increase at
120 Hz. The small positive thermal differences can also include mode order and
thermal carryover. CPU use rises modestly; neither CPU work nor game speed doubles.

For the same two mGBA GBA scenes, run-ahead has a much larger cost:

| Scene | Mode | CPU % | Average frequency | Temperature | Game FPS |
| --- | --- | ---: | ---: | ---: | ---: |
| Advance Wars | 60 Hz | 87.67 | 989.2 MHz | 56.70°C | 59.741 |
| Advance Wars | 120 Hz | 90.58 | 982.0 MHz | 56.65°C | 59.734 |
| Advance Wars | 60 Hz + run-ahead | 114.75 | 1,490.8 MHz | 60.96°C | 59.305 |
| Power Tour | 60 Hz | 80.83 | 880.9 MHz | 55.74°C | 59.721 |
| Power Tour | 120 Hz | 85.58 | 866.4 MHz | 56.18°C | 59.722 |
| Power Tour | 60 Hz + run-ahead | 102.17 | 1,498.5 MHz | 60.57°C | 59.574 |

Run-ahead pushes the governor near its maximum, adds roughly 4–5°C in these
comparisons and loses a small amount of game speed. Four Power Tour run-ahead
runs reported missed periods. Three Advance Wars run-ahead runs had nonzero
audio-starvation deltas; two lacked a pre-window periodic audio report, so their
fault deltas are unknown. They are not counted as clean runs.

A separate single 10-second check with both 120 Hz and run-ahead enabled
measured 120.20 Hz scanout, 59.30 game FPS, 118% CPU, and 1,512 MHz. It also
reported audio starvation. This confirms that the combination works, with the
same CPU-budget limitation seen at 60 Hz with run-ahead. It is not pooled into
the repeated comparison above. Its [raw measurements](results/rgsp-120-runahead-2026-10-15/results.json)
are retained separately. Run-Ahead therefore remains independent and off by
default, while supported panels use 120 Hz automatically.

## Software input response

Each mode has 120 successful trials across two runs; all 360 trials responded
within the 250 ms timeout. Values are pooled across the raw trials.

| Mode | Median | 95th percentile | Range | Separate run medians |
| --- | ---: | ---: | ---: | ---: |
| 60 Hz, no run-ahead | 41.26 ms | 48.99 ms | 30.36–50.03 ms | 41.09–41.39 ms |
| 120 Hz, no run-ahead | 32.76 ms | 40.64 ms | 22.33–42.66 ms | 32.27–33.95 ms |
| 60 Hz, one-frame run-ahead | 30.73 ms | 46.42 ms | 12.90–49.63 ms | 26.67–34.86 ms |

This measures software evdev injection to EGL swap return for the first changed
game image. It excludes switch debounce, physical-button sampling, LCD scanout
and pixel response. The original test ROM deliberately displays the previous
frame's A-button state so one-frame run-ahead has a delay it can remove. Real
games have different internal latency.

120 Hz consistently removes about one 120 Hz display interval from this path;
normal input and emulation still run at native game cadence. It does not halve
total input latency. Run-ahead sometimes responds earlier, but its run medians
vary substantially and its upper tail is worse than 120 Hz here. The frontend's
existing 12 ms worker wait can reuse the previous image when speculative work
finishes late. These results do not establish that run-ahead is consistently
faster than 120 Hz in actual games.

## Implementation and core findings

The previous pacing path fed presentation timing into emulation/audio timing.
The new frontend uses absolute native game deadlines from the core's AV info;
physical display refresh does not set game speed or source audio rate. A slightly
slower original panel can require two committed frames before one presentation
to preserve elapsed game time. Recovery from renderer stalls avoids committing
the same elapsed time twice. Fast-forward, rewind and link paths have automated
coverage; the live resource matrix measures normal single-player emulation.

Slot's run-ahead is one speculative frame on the same core: commit a frame,
serialize, execute the extra frame with the same input, retain its video/audio,
and restore the committed state. This roughly doubles core execution and adds
state-copy work; total process CPU percentage does not simply double because
the governor raises frequency and frontend work is shared. Eligibility is
GBA/mGBA only, enforced by `app::runahead_for`. Commit
`4a4ba92c51b5af3d77d288309cc28938ac5e86f3` introduced it as a one-frame mGBA GBA
setting. Its code/logs do not explain the gpSP exclusion; that exclusion alone
does not prove gpSP incapable of run-ahead.

The device's original gpSP core had two startup crashes in the JIT cache path
during 120 Hz tests, before the emulator reached its pacing loop. Its build
predated the repository's existing `jit-cache-in-binary.patch`. Rebuilding the
pinned core with the current Cortex-A53 recipe and both existing patches removes
the unchecked mmap allocation from that path. All 16 final gpSP runs succeeded
with this corrected build. Old-core runs and crashes are archived separately and
are not pooled into these comparisons. Both updated Slot and gpSP are installed
on the tested device, with originals backed up locally.

## Artifacts

- [Measurement procedure and rerun commands](README.md).
- [Checked summary, ranges and diagnostics](results/rgsp-2026-10-15/summary.md).
- [Per-run measurements](results/rgsp-2026-10-15/results.json),
  [exact plan](results/rgsp-2026-10-15/plan.json) and
  [device/build metadata](results/rgsp-2026-10-15/metadata.json).
- Raw latency CSVs and LCD status files are alongside the summary. Full raw
  performance CSVs and logs are retained in `/private/tmp/slot-120hz/matrix`.

The recorded profile binary has the earlier menu toggle. The final default,
menu and model-allowlist revision preserves the same timing and emulation
implementation; the sampling matrix was not repeated for those changes.
