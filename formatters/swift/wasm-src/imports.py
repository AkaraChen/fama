#!/usr/bin/env python3
"""List the import table of a core wasm module (probe gate: must be WASI-only)."""
import re
import sys

data = open(sys.argv[1], "rb").read()
assert data[:4] == b"\x00asm", "not a wasm module"
pos = 8

def leb(d, i):
    r = 0
    s = 0
    while True:
        b = d[i]
        i += 1
        r |= (b & 0x7F) << s
        if not b & 0x80:
            return r, i
        s += 7

imports = []
try:
    while pos < len(data):
        sid = data[pos]
        size, pos = leb(data, pos + 1)
        end = pos + size
        if sid == 2:
            n, p = leb(data, pos)
            for _ in range(n):
                ml, p = leb(data, p)
                mod = data[p : p + ml].decode("utf-8", "replace")
                p += ml
                fl, p = leb(data, p)
                fld = data[p : p + fl].decode("utf-8", "replace")
                p += fl
                kind = data[p]
                p += 1
                if kind == 0:  # func
                    _, p = leb(data, p)
                elif kind == 1:  # table: reftype, flags, [limits], init-expr
                    p += 2
                    # skip limit if flag has bounds bits
                    # conservative: scan to 0x0B (end opcode)
                    while data[p] != 0x0B:
                        p += 1
                    p += 1
                elif kind == 2:  # memory
                    flg = data[p]
                    p += 1
                    _, p = leb(data, p)
                    if flg & 1:
                        _, p = leb(data, p)
                elif kind == 3:  # global
                    p += 2
                else:
                    raise ValueError(f"kind {kind}")
                imports.append((mod, fld, kind))
        pos = end
except Exception as e:  # strict parse tripped: keep partial, flag it
    print(f"[parse warning: {e} after {len(imports)} imports]")

by_mod = {}
for mod, fld, kind in imports:
    by_mod.setdefault(mod, []).append(fld)

print(f"total imports: {len(imports)}")
for mod in sorted(by_mod):
    names = sorted(set(by_mod[mod]))
    print(f"  {mod}: {len(by_mod[mod])} imports, e.g. {', '.join(names[:8])}")

js = [f"{m}::{f}" for m, f, _ in imports if "js" in m.lower() or "javascript" in f.lower()]
nonwasi = [m for m in by_mod if m != "wasi_snapshot_preview1"]
print("JS_IMPORTS:" , js if js else "NONE")
print("NON_WASI_MODULES:", sorted(set(nonwasi)) if nonwasi else "NONE")
