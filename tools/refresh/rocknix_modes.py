#!/usr/bin/env python3
"""Decode ROCKNIX panel firmware timings using its panel-mipi.c file format."""

import argparse
import json
import pathlib
import struct


def modes(path):
    data = path.read_bytes()
    if len(data) < 64 or data[:16] != b"PANEL-FIRMWARE\0\x01":
        raise ValueError(f"{path}: unsupported panel firmware header")
    preferred, count = data[62:64]
    if not count or preferred >= count or len(data) < 64 + count * 32:
        raise ValueError(f"{path}: invalid timing table")
    timings = []
    for index in range(count):
        values = struct.unpack_from(">8H2I", data, 64 + index * 32)
        htotal, vtotal = sum(values[:4]), sum(values[4:8])
        if not values[0] or not values[4]:
            raise ValueError(f"{path}: empty mode {index}")
        clock = values[8] or htotal * vtotal * 60 // 1000
        timings.append(dict(width=values[0], height=values[4], htotal=htotal,
                            vtotal=vtotal, clock_khz=clock,
                            hz=round(clock * 1000 / htotal / vtotal, 4)))
    return dict(file=path.name, preferred=preferred, modes=timings)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=pathlib.Path)
    parser.add_argument("--revision", required=True, help="ROCKNIX distribution commit")
    args = parser.parse_args()
    paths = sorted(args.directory.glob("*.panel"))
    if not paths:
        parser.error("no .panel files found")
    print(json.dumps(dict(revision=args.revision, panels=[modes(path) for path in paths]), indent=2))


if __name__ == "__main__":
    main()
