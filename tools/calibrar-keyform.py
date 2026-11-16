#!/usr/bin/env python3
"""Compara cada keyform de cada malla contra la geometría que entrega el Core
(el dump del oráculo). Objetivo: descubrir la regla de selección de keyform.

No copia nada del Core: solo compara números.
"""
import json, sys
from moc3 import Moc3

moc_path = sys.argv[1]
json_path = sys.argv[2]
m = Moc3.from_file(moc_path)
g = m.get

kf_xys = g("keyform_position.xys")
kf_begin = g("art_mesh.keyform_begin_indices")
kf_count = g("art_mesh.keyform_counts")
kf_band = g("art_mesh.keyform_binding_band_indices")
bands_count = g("keyform_binding_band.counts")
pos_begin = g("art_mesh_keyform.keyform_position_begin_indices")
idx = g("position_index.indices")
pib = g("art_mesh.position_index_begin_indices")
vc = g("art_mesh.vertex_counts")
parent = g("art_mesh.parent_deformer_indices")

dump = json.load(open(json_path))
ppu = dump["canvas"]["ppu"]
draw = dump["drawables"]

print(f"{'malla':>5} {'kf':>3} {'banda':>5} {'claves':>6} {'parent':>6} {'mejor':>5} "
      f"{'err_px':>8} {'err_u':>8} {'escala':>7}")
print("-" * 62)

aciertos = {"px": [], "u": []}
for i in range(min(len(draw), len(kf_begin))):
    n_kf = kf_count[i]
    if n_kf == 0:
        continue
    # índices de esta malla -> número real de vértices
    sl = idx[pib[i]:pib[i] + vc[i]]
    n_verts = (max(sl) + 1) if sl else 0
    if n_verts == 0:
        continue
    obj = draw[i]["pos"]  # unidades, 2 por vértice
    if len(obj) < 2 * n_verts:
        continue

    b = kf_band[i]
    claves = bands_count[b] if b < len(bands_count) else 0

    best = []
    for k in range(n_kf):
        # offset real del bloque de posiciones de este keyform
        ki = kf_begin[i] + k
        if ki >= len(pos_begin):
            continue
        base = pos_begin[ki]
        if base + 2 * n_verts > len(kf_xys):
            continue
        # error en px (raw tal cual) y en unidades (raw/ppu)
        e_px = e_u = 0.0
        for v in range(n_verts):
            ax, ay = kf_xys[base + 2 * v], kf_xys[base + 2 * v + 1]
            ox, oy = obj[2 * v] * ppu, obj[2 * v + 1] * ppu
            e_px += (ax - ox) ** 2 + (ay - oy) ** 2
            e_u += (ax / ppu - obj[2 * v]) ** 2 + (ay / ppu - obj[2 * v + 1]) ** 2
        best.append((e_px ** 0.5 / n_verts, e_u ** 0.5 / n_verts, k))

    if not best:
        continue
    bpx = min(best, key=lambda t: t[0])
    bu = min(best, key=lambda t: t[1])
    aciertos["px"].append(bpx[2])
    aciertos["u"].append(bu[2])

    if i < 14 or bu[1] < 1e-3:
        print(f"{i:>5} {n_kf:>3} {b:>5} {claves:>6} {parent[i]:>6} {bu[2]:>5} "
              f"{bpx[0]:>8.2f} {bu[1]:>8.5f} {'u' if bu[1] < bpx[0] else 'px':>7}")

from collections import Counter
print("\nkeyform ganador (escala unidades):", Counter(aciertos["u"]).most_common(6))
print("keyform ganador (escala px)      :", Counter(aciertos["px"]).most_common(6))
print("coinciden ambos criterios         :",
      sum(1 for a, b in zip(aciertos['u'], aciertos['px']) if a == b), "de", len(aciertos['u']))
