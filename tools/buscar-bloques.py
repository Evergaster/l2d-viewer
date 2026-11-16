"""Localiza el bloque de posiciones de cada malla en el pool con un ajuste rígido
(escala uniforme + rotación + traslación, sin reflexión). Un ajuste afín libre acepta
demasiados bloques; uno rígido solo cede el que de verdad es.

Uso: python3 tools/buscar-bloques.py <volcado.json> <oracle.json>
"""
import json
import sys

import numpy as np


def ajustar(b, obj):
    """Umeyama: devuelve (err_max, escala, ángulo_grados)."""
    mb, mo = b.mean(0), obj.mean(0)
    bc, oc = b - mb, obj - mo
    cov = oc.T @ bc / len(b)
    u, s, vt = np.linalg.svd(cov)
    d = np.sign(np.linalg.det(u) * np.linalg.det(vt))
    r = u @ np.diag([1.0, d]) @ vt
    var_b = (bc ** 2).sum() / len(b)
    escala = float((s * np.array([1.0, d])).sum() / var_b) if var_b > 0 else 0.0
    pred = (escala * (r @ bc.T).T) + mo
    err = np.abs(pred - obj).max()
    ang = np.degrees(np.arctan2(r[1, 0], r[0, 0]))
    return err, escala, ang


def main():
    d = json.load(open(sys.argv[1]))
    o = json.load(open(sys.argv[2]))
    xys = np.array(d["keyform_position.xys"], dtype=np.float64).reshape(-1, 2)
    ids = d["art_mesh.ids"]
    kfb = d["art_mesh.keyform_begin_indices"]
    pb = d["art_mesh_keyform.keyform_position_begin_indices"]
    por = {x["id"]: np.array(x["pos"], dtype=np.float64).reshape(-1, 2) for x in o["drawables"]}
    cand = sorted(set(pb))

    filas = []
    for m, nombre in enumerate(ids):
        obj = por[nombre]
        n = len(obj)
        mejor = None
        for off in cand:
            i = off // 2
            if i + n > len(xys):
                continue
            e, s, a = ajustar(xys[i:i + n], obj)
            if mejor is None or e < mejor[0]:
                mejor = (e, off, s, a)
        filas.append((mejor[0], nombre, m, mejor[1], mejor[2], mejor[3]))

    filas.sort()
    print(f"{'err_max':>9} {'malla':>12} {'winner':>9} {'pb[kf0]':>8} {'dx':>7} {'escala':>8} {'ángulo':>7}")
    for e, nombre, m, off, s, a in filas:
        print(f"{e:9.5f} {nombre:>12} {off:9} {pb[kfb[m]]:8} {off - pb[kfb[m]]:7} {s:8.4f} {a:7.2f}")

    ok = [f for f in filas if f[0] < 1e-4]
    print(f"\ncuadran con ajuste rígido: {len(ok)}/{len(filas)}")
    if ok:
        print(f"  escala mediana {np.median([f[4] for f in ok]):.6f}, "
              f"ángulo mediano {np.median([f[5] for f in ok]):.2f}°")
        desde_pb = sum(1 for f in ok if f[3] == pb[kfb[f[2]]])
        print(f"  el ganador es exactamente pb[kf_begin]: {desde_pb}/{len(ok)}")


if __name__ == "__main__":
    main()
