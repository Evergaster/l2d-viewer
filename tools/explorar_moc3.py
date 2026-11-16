#!/usr/bin/env python3
"""Explorador del .moc3: recorre la tabla de offsets por sección y valida.

Herramienta de descubrimiento, no parte del producto. Se usa para deducir el layout real
antes de portarlo a Rust.
"""
import struct
import sys

U32 = lambda b, o: struct.unpack_from("<I", b, o)[0]
I32 = lambda b, o: struct.unpack_from("<i", b, o)[0]

# (nombre, multiplicador de N por cada array de offsets).
# OJO: el espacio de runtime NO se guarda en el archivo (solo existe en memoria),
# así que aquí no aparece ningún array de 8N.
LAYOUT = [
    ("parts",      [1, 1, 1, 1, 1, 1, 1]),
    ("deformers",  [1, 1, 1, 1, 1, 1, 1, 1]),
    ("warp",       [1, 1, 1, 1, 1, 1]),
    ("rotation",   [1, 1, 1, 1]),
    ("artmesh",    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]),
    ("params",     [1, 1, 1, 1, 1, 1, 1, 1]),
    ("partkf",     [1]),
    ("warpkf",     [1, 1]),
    ("rotkf",      [1, 1, 1, 1, 1, 1, 1]),
    ("artkf",      [1, 1, 1]),
]


def load(path):
    b = open(path, "rb").read()
    assert b[:4] == b"MOC3", "no es moc3"
    version = b[4]
    ci = U32(b, 0x40)
    ncounts = 23 if version <= 3 else 32 if version == 4 else 35
    counts = list(struct.unpack_from("<%dI" % ncounts, b, ci))
    return b, version, ci, counts


ALL_NAMES = ["parts", "deformers", "warp", "rotation", "artmesh", "params", "partkf", "warpkf",
             "rotkf", "artkf", "kfpos", "pbindex", "kfbind", "pbind", "keys", "uvs", "posidx",
             "masks", "dog", "dogo", "glue", "glueinfo", "gluekf"]


def walk(b, counts):
    """Devuelve {seccion: [arrays de offsets]} usando el layout de la tabla."""
    n = dict(zip(ALL_NAMES, counts))

    cursor = 0x48
    out = {}
    for name, mults in LAYOUT:
        count = n[name]
        fields = []
        for m in mults:
            total = count * m
            offs = list(struct.unpack_from("<%dI" % total, b, cursor)) if total else []
            fields.append(offs)
            cursor += total * 4
        out[name] = fields
    return out, cursor


def cstr(b, off):
    end = b.index(b"\x00", off)
    return b[off:end].decode("utf-8", "replace")


def main(path):
    b, version, ci, counts = load(path)
    print(f"{path}: v{version}, {len(b)} bytes")
    print("counts:", {k: v for k, v in zip(
        ["parts","deformers","warp","rotation","artmesh","params","partkf","warpkf","rotkf","artkf",
         "kfpos","uvs","posidx","masks"], counts)})
    sec, end = walk(b, counts)
    print(f"tabla consumida hasta {end:#x}")

    for name, idx in (("parts", 0), ("deformers", 0), ("artmesh", 0), ("params", 0)):
        offs = sec[name][idx][:5]
        print(f"\n{name} ids ({len(sec[name][idx])}):")
        for o in offs:
            try:
                print(f"  {o:#08x}  {cstr(b, o)!r}")
            except Exception as e:
                print(f"  {o:#08x}  <error {e}>")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "models/samples/Samples/Resources/Wanko/Wanko.moc3")
