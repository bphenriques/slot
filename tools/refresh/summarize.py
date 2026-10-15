#!/usr/bin/env python3
"""Summarize profile.py artifacts; strict validation exits nonzero after reporting problems."""
import argparse
import csv
import json
import math
import pathlib
import statistics
import sys
from collections import defaultdict


def percentile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (position - lower)


def spread(values, digits=2):
    if not values:
        return "—"
    return f"{statistics.median(values):.{digits}f} [{min(values):.{digits}f}, {max(values):.{digits}f}]"


def table(headers, rows):
    return ["| " + " | ".join(headers) + " |",
            "| " + " | ".join("---" for _ in headers) + " |"] + [
                "| " + " | ".join(map(str, row)) + " |" for row in rows]


def summarize(directory, expected):
    results = json.loads((directory / "results.json").read_text())
    if not isinstance(results, list):
        raise ValueError("results.json must contain a list")
    errors, groups, seen = [], defaultdict(list), set()

    def number(value, context):
        try:
            value = float(value)
            if not math.isfinite(value):
                raise ValueError()
            return value
        except (ValueError, TypeError):
            errors.append(f"{context}: missing or nonfinite number")
            return None

    for result in results:
        scene, mode = result["scene"], result["mode"]
        repeat = result["repeat"]
        if not isinstance(repeat, int) or isinstance(repeat, bool) or repeat < 1:
            errors.append(f"{scene}/{mode}: invalid repeat {repeat!r}")
            continue
        key = (bool(result["latency"]), scene, mode, repeat)
        if key in seen:
            errors.append(f"duplicate result: {key}")
            continue
        seen.add(key)
        groups[key[:3]].append(result)
        for name, value in result.items():
            if isinstance(value, (float, int)) and not isinstance(value, bool):
                number(value, f"{scene}/{mode}/{repeat}/{name}")

    plan_path = directory / "plan.json"
    if plan_path.is_file():
        plan = json.loads(plan_path.read_text())
        planned = set()
        for case in plan["cases"]:
            key = (case["latency"], case["scene"], case["mode"], case["repeat"])
            if not isinstance(key[0], bool) or not isinstance(key[3], int) or isinstance(key[3], bool) or key[3] < 1:
                raise ValueError(f"invalid planned case: {key}")
            if key in planned:
                errors.append(f"duplicate planned case: {key}")
            planned.add(key)
            groups.setdefault(key[:3], [])
        for key in sorted(planned - seen):
            errors.append(f"missing planned case: {key}")
        for key in sorted(seen - planned):
            errors.append(f"unexpected case outside plan: {key}")
    else:
        # Older artifacts do not record custom --scenes selections.
        for latency, scene, _ in list(groups):
            modes = ["60", "120"]
            if latency or (scene.startswith("gba-") and scene.endswith("mgba")):
                modes.append("runahead")
            for mode in modes:
                groups.setdefault((latency, scene, mode), [])
        for key, runs in groups.items():
            actual = {run["repeat"] for run in runs}
            wanted = set(range(1, expected + 1))
            if actual != wanted:
                errors.append(f"{key}: missing repeats {sorted(wanted - actual)}; unexpected repeats {sorted(actual - wanted)}")

    if plan_path.is_file():
        planned_counts = defaultdict(int)
        for key in planned:
            planned_counts[key[:3]] += 1
        counts = ", ".join(f"{count} runs for {sum(n == count for n in planned_counts.values())} groups"
                           for count in sorted(set(planned_counts.values())))
        expectation = f"Expected {len(planned)} exact cases from plan.json ({counts}); repeat sets follow the plan, not --expected-repeats."
    else:
        expectation = f"Expected {expected} run(s) per comparison group."
    lines = ["# Refresh measurements", "",
             expectation + " Runs columns show completed runs. Values are median [minimum, maximum] across runs.",
             "CPU is process CPU percentage (100% = one core); MHz follows the device governor.",
             "Temperature is each run's mean, not a steady-state thermal or battery measurement.", ""]
    performance = {key: runs for key, runs in groups.items() if not key[0]}
    perf_rows, health_rows = [], []
    metrics = [("game_fps", 3), ("cpu_percent", 2), ("clock_mhz", 1), ("temp_c", 2)]

    def values(runs, metric):
        return [value for run in runs
                if (value := number(run.get(metric), f"{run['scene']}/{run['mode']}/{run['repeat']}/{metric}")) is not None]

    for (_, scene, mode), runs in sorted(performance.items()):
        perf_rows.append([scene, mode, len(runs)] + [spread(values(runs, name), digits) for name, digits in metrics])
    lines += table(["Scene", "Mode", "Runs", "Game fps", "CPU %", "MHz", "Temperature °C"], perf_rows) + [""]
    pairs = []
    for scene in sorted({key[1] for key in performance}):
        sides = [{r["repeat"]: r for r in performance.get((False, scene, mode), [])} for mode in ("60", "120")]
        shared = sorted(sides[0].keys() & sides[1].keys())
        deltas = []
        for metric, digits in metrics:
            delta = []
            for repeat in shared:
                a = number(sides[0][repeat].get(metric), f"{scene}/60/{repeat}/{metric}")
                b = number(sides[1][repeat].get(metric), f"{scene}/120/{repeat}/{metric}")
                if a is not None and b is not None:
                    delta.append(b - a)
            deltas.append(spread(delta, digits))
        pairs.append([scene, len(shared)] + deltas)
    lines += ["## Paired 120 Hz minus 60 Hz", ""]
    lines += table(["Scene", "Pairs", "Δ game fps", "Δ CPU percentage points", "Δ MHz", "Δ °C"], pairs) + [""]

    for (latency, scene, mode), runs in sorted(groups.items()):
        faults = []
        for metric in ("audio_drop_delta", "audio_starve_delta"):
            zero = nonzero = missing = 0
            for run in runs:
                if metric not in run:
                    missing += 1
                else:
                    value = number(run[metric], f"{scene}/{mode}/{run['repeat']}/{metric}")
                    if value is None:
                        missing += 1
                    elif value == 0:
                        zero += 1
                    else:
                        nonzero += 1
                        if value < 0:
                            errors.append(f"{scene}/{mode}: negative {metric}")
            faults.append(f"{zero}/{nonzero}/{missing}")
        rates = [v for run in runs if "display_hz" in run
                 if (v := number(run["display_hz"], f"{scene}/{mode}/display_hz")) is not None]
        health_rows.append([scene + (" (latency)" if latency else ""), mode] + faults +
                           [sum(bool(r.get("timing_warning")) for r in runs),
                            f"{min(rates):.3f}–{max(rates):.3f}" if rates else "missing"])
    lines += ["## Diagnostics", "", "Audio columns count runs with zero / nonzero / missing fault deltas.",
              "Fault deltas bracket periodic log reports; their boundaries may extend beyond the timed window.", ""]
    lines += table(["Scene", "Mode", "Audio drops", "Audio starvation", "Timing warnings", "LCD Hz range"], health_rows) + [""]

    latency_groups = defaultdict(list)
    for (latency, scene, mode), runs in groups.items():
        if latency:
            latency_groups[mode].extend(runs)
    latency_rows = []
    for mode, runs in sorted(latency_groups.items()):
        pooled, medians, failures, trials = [], [], 0, 0
        for run in runs:
            path = directory / f"{run['scene']}-{mode}-{run['repeat']}-latency.csv"
            if not path.is_file():
                errors.append(f"missing raw latency CSV: {path.name}")
                continue
            with path.open(newline="") as source:
                rows = list(csv.DictReader(source))
            trial_ids, accepted = set(), []
            for row in rows:
                trials += 1
                trial = row.get("trial")
                if trial in trial_ids:
                    errors.append(f"{path.name}: duplicate trial {trial}")
                trial_ids.add(trial)
                value = number(row.get("latency_ms"), f"{path.name}/trial {trial}")
                if value is None or value < 0 or value > 250:
                    failures += 1
                    if value is not None:
                        errors.append(f"{path.name}/trial {trial}: latency outside 0..250 ms")
                else:
                    accepted.append(value)
            if len(rows) != run.get("trials"):
                errors.append(f"{path.name}: raw trial count {len(rows)} differs from results.json {run.get('trials')}")
            if accepted:
                medians.append(statistics.median(accepted))
                pooled.extend(accepted)
            else:
                errors.append(f"{path.name}: no successful latency trials")
        latency_rows.append([mode, len(runs), spread(medians, 3),
                             f"{percentile(pooled, .5):.3f}" if pooled else "—",
                             f"{percentile(pooled, .95):.3f}" if pooled else "—",
                             f"{min(pooled):.3f}–{max(pooled):.3f}" if pooled else "—",
                             len(pooled), failures, trials])
    lines += ["## Software response latency", "",
              "Injection to swap return of the first responding controlled-ROM game image; excludes physical-switch and photon latency.",
              "Pooled percentiles use linear interpolation across successful raw trials; failed/nonfinite trials invalidate a strict report.", ""]
    lines += table(["Mode", "Runs", "Run medians ms", "Pooled p50 ms", "Pooled p95 ms", "Min–max ms", "Successes", "Failures", "Trials"], latency_rows) + [""]
    errors = list(dict.fromkeys(errors))
    lines += ["## Validation", ""]
    lines += [f"- {error}" for error in errors] if errors else ["All observed comparison groups have complete, finite, nonduplicate results."]
    coverage = (f"Coverage checked against all {len(planned)} cases in plan.json." if plan_path.is_file()
                else "Groups for entirely absent scenes cannot be inferred from results.json without plan.json.")
    lines += ["", coverage + " No performance winner is selected by this summary."]
    return "\n".join(lines) + "\n", errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=pathlib.Path)
    parser.add_argument("--expected-repeats", type=int, default=5)
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--partial", action="store_true", help="report validation problems without failing")
    args = parser.parse_args()
    if args.expected_repeats < 1:
        parser.error("--expected-repeats must be positive")
    try:
        text, errors = summarize(args.directory, args.expected_repeats)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"summarize: {error}\n")
    if args.output:
        args.output.write_text(text)
    else:
        print(text, end="")
    if errors and not args.partial:
        sys.exit(1)


if __name__ == "__main__":
    main()
