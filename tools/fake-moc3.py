#!/usr/bin/env python3
"""Genera un .moc3 sintético (contenedor válido, sin datos de malla reales).

Sirve para probar el lector sin usar un modelo de Live2D. No contiene nada de Live2D:
solo rellena el layout descrito en docs/03-formato-moc3.md.

uso: python3 tools/fake-moc3.py [salida.moc3] [version]
"""
import struct
import sys

BASE = ["parts", "deformers", "warp_deformers", "rotation_deformers", "art_meshes",
        "parameters", "part_keyforms", "warp_deformer_keyforms", "rotation_deformer_keyforms",
        "art_mesh_keyforms", "keyform_positions", "parameter_binding_indices",
        "keyform_bindings", "parameter_bindings", "keys", "uvs", "position_indices",
        "drawable_masks", "draw_order_groups", "draw_order_group_objects", "glue",
        "glue_info", "glue_keyforms"]

BASE_VALS = [12, 10, 5, 5, 37, 48, 12, 25, 25, 180, 900, 400, 300, 120, 90, 1400, 5000,
             40, 3, 9, 2, 2, 6]

V4 = ["keyform_multiply_colors", "keyform_screen_colors", "blend_shape_parameter_bindings",
      "blend_shape_keyform_bindings", "blend_shapes_warp_deformers", "blend_shapes_art_meshes",
      "blend_shape_constraint_indices", "blend_shape_constraints",
      "blend_shape_constraint_values"]
V4_VALS = [180, 180, 60, 60, 8, 8, 20, 20, 60]

V5 = ["blend_shapes_parts", "blend_shapes_rotation_deformers", "blend_shapes_glue"]
V5_VALS = [4, 4, 2]

TABLE = 0x2C0


def build(version: int) -> bytes:
    counts = dict(zip(BASE, BASE_VALS))
    if version >= 4:
        counts.update(dict(zip(V4, V4_VALS)))
    if version >= 5:
        counts.update(dict(zip(V5, V5_VALS)))

    # Orden de la tabla de secciones, con las repeticiones que exige el formato
    # (varias secciones comparten contador con la base).
    seq = list(BASE)
    if version >= 2:
        seq.append("warp_deformers")
    if version >= 4:
        seq += ["parameters", "warp_deformers", "rotation_deformers", "art_meshes",
                "keyform_multiply_colors", "keyform_screen_colors", "parameters",
                "blend_shape_parameter_bindings", "blend_shape_keyform_bindings",
                "blend_shapes_warp_deformers", "blend_shapes_art_meshes",
                "blend_shape_constraint_indices", "blend_shape_constraints",
                "blend_shape_constraint_values"]
    if version >= 5:
        seq += ["warp_deformer_keyforms", "rotation_deformer_keyforms", "art_mesh_keyforms",
                "blend_shapes_parts", "blend_shapes_rotation_deformers", "blend_shapes_glue"]

    rows = []
    for name in seq:
        rows += [name] * counts[name]

    table_end = TABLE + 8 + 4 * len(rows)
    count_info = (table_end + 15) & ~15
    pad = 256 if version >= 5 else 128
    canvas = count_info + pad + 0x100

    buf = bytearray(canvas + 0x100)
    buf[0:4] = b"MOC3"
    buf[4] = version
    buf[5] = 0
    struct.pack_into("<I", buf, TABLE, count_info)
    struct.pack_into("<I", buf, TABLE + 4, canvas)

    for i, f in enumerate([2.0, 10.0, 20.0, 800.0, 600.0]):
        struct.pack_into("<f", buf, canvas + i * 4, f)

    order = BASE + ([n for n in V4] if version >= 4 else []) + ([n for n in V5] if version >= 5 else [])
    for i, name in enumerate(order):
        struct.pack_into("<I", buf, count_info + pad + i * 4, counts[name])

    cursor = TABLE + 8
    for _ in rows:
        struct.pack_into("<I", buf, cursor, 0x3000)
        cursor += 4

    return bytes(buf)


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "fake.moc3"
    version = int(sys.argv[2]) if len(sys.argv) > 2 else 4
    data = build(version)
    with open(out, "wb") as fh:
        fh.write(data)
    print(f"{out}: {len(data)} bytes, versión {version}")
