#!/usr/bin/env python3
"""Build a delayed-A red/green test ROM. Optional --header copies its GBA logo."""
import argparse
import pathlib
import struct
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("output", type=pathlib.Path)
p.add_argument("--header", type=pathlib.Path)
p.add_argument("--clang", default="clang")
a = p.parse_args()
with tempfile.TemporaryDirectory() as d:
    obj = pathlib.Path(d) / "response.o"
    subprocess.run([a.clang, "-target", "armv4t-none-eabi", "-c",
                    str(pathlib.Path(__file__).with_name("response.s")), "-o", str(obj)], check=True)
    elf = obj.read_bytes()
    assert elf[:6] == b"\x7fELF\x01\x01", "expected little-endian ELF32"
    shoff = struct.unpack_from("<I", elf, 32)[0]
    size, count, names = struct.unpack_from("<HHH", elf, 46)
    sections = [struct.unpack_from("<10I", elf, shoff + i * size) for i in range(count)]
    strings = elf[sections[names][4]:][:sections[names][5]]
    text = None
    for s in sections:
        name = strings[s[0]:].split(b"\0", 1)[0]
        if name in (b".rel.text", b".rela.text") and s[5]:
            raise RuntimeError("unexpected relocations in test ROM")
        if name == b".text":
            text = elf[s[4]:][:s[5]]
    assert text, "missing .text"
header = bytearray(0xc0)
struct.pack_into("<I", header, 0, 0xea00002e)  # ARM branch to 0xc0
if a.header:
    source = a.header.read_bytes()
    if len(source) < 0xa0:
        raise ValueError("header source must include bytes 0..0x9f")
    header[4:0xa0] = source[4:0xa0]
header[0xa0:0xac] = b"SLOTRESP    "
header[0xac:0xb0] = b"SLTE"
header[0xb0:0xb2] = b"00"
header[0xb2] = 0x96
header[0xbd] = (-sum(header[0xa0:0xbd]) - 0x19) & 255
a.output.write_bytes(header + text + bytes(max(0, 0x8000 - len(header) - len(text))))
print(a.output)
