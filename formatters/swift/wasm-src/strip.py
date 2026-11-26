#!/usr/bin/env python3
"""Strip all custom sections (name, DWARF debug, producers) from a wasm file.

A wasm module remains fully valid without custom sections; they are ~36% of
the swift-format module's bytes and only serve debuggers/provenance."""
import sys

path = sys.argv[1]
data = open(path, "rb").read()
assert data[:4] == b"\x00asm", f"{path}: not a wasm module"


def leb(d, i):
    r = s = 0
    while True:
        b = d[i]
        i += 1
        r |= (b & 0x7F) << s
        if not b & 0x80:
            return r, i
        s += 7


out = [data[:8]]
pos, dropped = 8, 0
while pos < len(data):
    sid = data[pos]
    size, hdr_end = leb(data, pos + 1)
    end = hdr_end + size
    if sid == 0:
        dropped += 1
    else:
        out.append(data[pos:end])
    pos = end

open(path, "wb").write(b"".join(out))
print(
    f"stripped {dropped} custom sections: {len(data)} -> {len(b''.join(out))} bytes"
)
