# Refresh measurements

Expected 60 exact cases from plan.json (2 runs for 7 groups, 3 runs for 2 groups, 5 runs for 8 groups); repeat sets follow the plan, not --expected-repeats. Runs columns show completed runs. Values are median [minimum, maximum] across runs.
CPU is process CPU percentage (100% = one core); MHz follows the device governor.
Temperature is each run's mean, not a steady-state thermal or battery measurement.

| Scene | Mode | Runs | Game fps | CPU % | MHz | Temperature °C |
| --- | --- | --- | --- | --- | --- | --- |
| gb-mgba | 120 | 2 | 59.723 [59.718, 59.728] | 80.79 [79.67, 81.92] | 622.2 [585.7, 658.7] | 54.03 [54.02, 54.04] |
| gb-mgba | 60 | 2 | 59.705 [59.692, 59.718] | 78.46 [78.08, 78.83] | 602.5 [588.7, 616.3] | 53.98 [53.96, 54.01] |
| gba-gpsp | 120 | 5 | 59.727 [59.723, 59.730] | 83.17 [82.50, 83.83] | 668.2 [566.9, 702.7] | 54.39 [53.98, 54.61] |
| gba-gpsp | 60 | 5 | 59.724 [59.694, 59.754] | 80.25 [79.58, 82.58] | 669.4 [581.2, 710.9] | 54.04 [53.66, 54.15] |
| gba-heavy-gpsp | 120 | 3 | 59.730 [59.724, 59.732] | 61.83 [61.50, 61.83] | 612.3 [512.9, 637.5] | 54.40 [54.30, 54.48] |
| gba-heavy-gpsp | 60 | 3 | 59.716 [59.715, 59.716] | 59.58 [59.50, 59.58] | 539.3 [510.9, 640.1] | 53.92 [53.92, 53.98] |
| gba-heavy-mgba | 120 | 5 | 59.722 [59.719, 59.731] | 85.58 [84.92, 86.25] | 866.4 [773.6, 870.9] | 56.18 [55.32, 56.33] |
| gba-heavy-mgba | 60 | 5 | 59.721 [59.712, 59.722] | 80.83 [80.67, 81.25] | 880.9 [819.8, 901.7] | 55.74 [55.01, 55.90] |
| gba-heavy-mgba | runahead | 5 | 59.574 [59.572, 59.690] | 102.17 [101.92, 102.50] | 1498.5 [1481.7, 1504.9] | 60.57 [60.37, 61.04] |
| gba-mgba | 120 | 5 | 59.734 [59.718, 59.740] | 90.58 [89.67, 90.92] | 982.0 [948.9, 1007.8] | 56.65 [56.01, 56.94] |
| gba-mgba | 60 | 5 | 59.741 [59.715, 59.755] | 87.67 [87.25, 88.50] | 989.2 [951.9, 1010.4] | 56.70 [55.60, 56.80] |
| gba-mgba | runahead | 5 | 59.305 [59.291, 59.377] | 114.75 [114.67, 115.67] | 1490.8 [1392.6, 1512.0] | 60.96 [60.57, 61.12] |
| gbc-mgba | 120 | 2 | 59.714 [59.709, 59.719] | 80.96 [80.58, 81.33] | 617.7 [587.9, 647.4] | 54.18 [53.99, 54.37] |
| gbc-mgba | 60 | 2 | 59.729 [59.705, 59.753] | 79.33 [79.17, 79.50] | 583.8 [553.4, 614.3] | 53.95 [53.89, 54.01] |

## Paired 120 Hz minus 60 Hz

| Scene | Pairs | Δ game fps | Δ CPU percentage points | Δ MHz | Δ °C |
| --- | --- | --- | --- | --- | --- |
| gb-mgba | 2 | 0.018 [0.010, 0.026] | 2.33 [0.83, 3.83] | 19.7 [-30.5, 70.0] | 0.04 [0.03, 0.06] |
| gba-gpsp | 5 | 0.003 [-0.028, 0.029] | 2.25 [1.17, 4.25] | -8.1 [-26.0, 33.7] | 0.35 [0.22, 0.58] |
| gba-heavy-gpsp | 3 | 0.014 [0.009, 0.016] | 2.25 [1.92, 2.33] | 2.0 [-27.8, 98.2] | 0.48 [0.38, 0.50] |
| gba-heavy-mgba | 5 | 0.004 [-0.001, 0.019] | 4.33 [3.92, 5.50] | -12.1 [-128.1, 51.2] | 0.44 [0.30, 0.50] |
| gba-mgba | 5 | -0.000 [-0.027, 0.003] | 2.42 [2.17, 3.08] | -2.6 [-52.0, 41.5] | 0.08 [-0.05, 0.42] |
| gbc-mgba | 2 | -0.015 [-0.034, 0.004] | 1.62 [1.08, 2.17] | 33.8 [-26.4, 94.0] | 0.23 [-0.02, 0.49] |

## Diagnostics

Audio columns count runs with zero / nonzero / missing fault deltas.
Fault deltas bracket periodic log reports; their boundaries may extend beyond the timed window.

| Scene | Mode | Audio drops | Audio starvation | Timing warnings | LCD Hz range |
| --- | --- | --- | --- | --- | --- |
| gb-mgba | 120 | 2/0/0 | 2/0/0 | 0 | 120.084–120.166 |
| gb-mgba | 60 | 2/0/0 | 2/0/0 | 0 | 59.583–59.583 |
| gba-gpsp | 120 | 5/0/0 | 5/0/0 | 0 | 120.083–120.166 |
| gba-gpsp | 60 | 5/0/0 | 5/0/0 | 0 | 59.583–59.583 |
| gba-heavy-gpsp | 120 | 3/0/0 | 3/0/0 | 0 | 120.082–120.083 |
| gba-heavy-gpsp | 60 | 3/0/0 | 3/0/0 | 0 | 59.583–59.583 |
| gba-heavy-mgba | 120 | 5/0/0 | 5/0/0 | 0 | 120.083–120.167 |
| gba-heavy-mgba | 60 | 5/0/0 | 5/0/0 | 0 | 59.583–59.583 |
| gba-heavy-mgba | runahead | 5/0/0 | 5/0/0 | 4 | 59.583–59.583 |
| gba-mgba | 120 | 5/0/0 | 5/0/0 | 0 | 120.083–120.167 |
| gba-mgba | 60 | 5/0/0 | 5/0/0 | 0 | 59.500–59.583 |
| gba-mgba | runahead | 3/0/2 | 0/3/2 | 0 | 59.583–59.583 |
| gbc-mgba | 120 | 2/0/0 | 2/0/0 | 0 | 120.083–120.166 |
| gbc-mgba | 60 | 2/0/0 | 2/0/0 | 0 | 59.583–59.583 |
| gba-mgba (latency) | 120 | 2/0/0 | 2/0/0 | 0 | missing |
| gba-mgba (latency) | 60 | 2/0/0 | 2/0/0 | 0 | missing |
| gba-mgba (latency) | runahead | 2/0/0 | 2/0/0 | 0 | missing |

## Software response latency

Injection to swap return of the first responding controlled-ROM game image; excludes physical-switch and photon latency.
Pooled percentiles use linear interpolation across successful raw trials; failed/nonfinite trials invalidate a strict report.

| Mode | Runs | Run medians ms | Pooled p50 ms | Pooled p95 ms | Min–max ms | Successes | Failures | Trials |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 120 | 2 | 33.112 [32.273, 33.952] | 32.761 | 40.644 | 22.329–42.663 | 120 | 0 | 120 |
| 60 | 2 | 41.242 [41.092, 41.391] | 41.260 | 48.985 | 30.363–50.028 | 120 | 0 | 120 |
| runahead | 2 | 30.761 [26.666, 34.855] | 30.726 | 46.424 | 12.895–49.627 | 120 | 0 | 120 |

## Validation

All observed comparison groups have complete, finite, nonduplicate results.

Coverage checked against all 60 cases in plan.json. No performance winner is selected by this summary.
