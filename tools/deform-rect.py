"""Mide el dominio de cada warp: el (u,v) que usa el Core es afín en su entrada.

Para cada malla y cada warp de su cadena se aplica el prefijo (deformadores de hoja hacia
arriba) e se invierte el sufijo (sólo rotaciones, ya validadas en docs/13). Con el par
`(a = entrada real, b = salida real)` se recupera el `(u,v)` invirtiendo la rejilla destino y
se ajusta `uv = A·a + c`. Después se compara el rectángulo resultante con los datos del archivo.

Uso: python3 tools/deform-rect.py [modelo.moc3] [oraculo.json]
"""
import importlib.util
import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parent.parent


def modulo():
    spec = importlib.util.spec_from_file_location("deform_buscar", RAIZ / "tools" / "deform-buscar.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MOD = modulo()


def aplicar_rot(mod, i, k, p):
    s = int(mod.spec[i])
    kk = int(mod.rot_kf_ini[s] + k)
    esc = float(mod.rot_esc[kk])
    R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
    if mod.reflect_x.size:
        p = p * np.array([-1.0 if mod.reflect_x[kk] else 1.0, -1.0 if mod.reflect_y[kk] else 1.0])
    return esc * (p @ R.T) + np.array([mod.rot_ox[kk], mod.rot_oy[kk]])


def invertir_rot(mod, i, k, p):
    s = int(mod.spec[i])
    kk = int(mod.rot_kf_ini[s] + k)
    esc = float(mod.rot_esc[kk])
    R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
    q = ((p - np.array([mod.rot_ox[kk], mod.rot_oy[kk]])) / esc) @ R
    if mod.reflect_x.size:
        q = q * np.array([-1.0 if mod.reflect_x[kk] else 1.0, -1.0 if mod.reflect_y[kk] else 1.0])
    return q


def invertir_rejilla(rej, pts, iteraciones=60):
    """(u,v) en celdas [0,5] tal que bilineal(rej, uv) = pts."""
    mn = rej.reshape(-1, 2).min(0)
    mx = rej.reshape(-1, 2).max(0)
    uv = np.clip((pts - mn) / np.maximum(mx - mn, 1e-12) * 5.0, 0.0, 5.0)
    h = 1e-5
    for _ in range(iteraciones):
        f = MOD.bilineal(rej, uv) - pts
        jx = (MOD.bilineal(rej, uv + [h, 0.0]) - MOD.bilineal(rej, uv - [h, 0.0])) / (2 * h)
        jy = (MOD.bilineal(rej, uv + [0.0, h]) - MOD.bilineal(rej, uv - [0.0, h])) / (2 * h)
        J = np.stack([jx, jy], axis=2)
        try:
            d = np.linalg.solve(J, f[..., None])[..., 0]
        except np.linalg.LinAlgError:
            break
        uv = np.clip(uv - d, 0.0, 5.0)
    return uv


def main():
    ruta = sys.argv[1] if len(sys.argv) > 1 else str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3")
    oraculo = sys.argv[2] if len(sys.argv) > 2 else str(
        RAIZ / "models/samples/hiyori.oracle.json")

    d = MOD.leer_moc3(ruta)
    mod = MOD.Modelo(d, json.load(open(oraculo)))
    por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
              for x in json.load(open(oraculo))["drawables"]}

    bnd = MOD.Bindings(d)
    kf_def = {}
    for i in range(mod.n_def):
        propio = (d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                  else d["rotation_deformer.keyform_binding_band_indices"])
        kf_def[i] = bnd.indice(int(propio[int(mod.spec[i])]), int(mod.n_kf[i]))
    kf_malla = {m: bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                              int(mod.kf_malla_cnt[m])) for m in range(mod.n_malla)}

    # --- medir (A, c) por warp ---
    por_warp = defaultdict(list)
    for m in range(mod.n_malla):
        cadena = mod.cadenas[m]
        for j, i in enumerate(cadena):
            if mod.tipos[i] != 0:
                continue
            sufijo = cadena[j + 1:]
            if any(mod.tipos[x] != 1 for x in sufijo):
                continue  # no se puede invertir un warp sin conocer su dominio
            p = mod.puntos_malla(m, kf_malla[m])
            for x in cadena[:j]:
                p = (MOD.bilineal(mod.rejillas[x][kf_def[x]], p * 5.0) if mod.tipos[x] == 0
                     else aplicar_rot(mod, x, kf_def[x], p))
            a = p
            b = por_id[mod.ids_malla[m]] * np.array([1.0, -1.0])
            for x in reversed(sufijo):
                b = invertir_rot(mod, x, kf_def[x], b)
            rej = mod.rejillas[i][kf_def[i]]
            uv = invertir_rejilla(rej, b)
            A = np.hstack([a, np.ones((len(a), 1))])
            sol, *_ = np.linalg.lstsq(A, uv, rcond=None)
            res = float(np.abs(A @ sol - uv).max())
            if res > 5e-3:
                continue
            por_warp[i].append((sol, res, a, m))

    print(f"warps medidos: {len(por_warp)} de {mod.n_warps if hasattr(mod,'n_warps') else mod.n_def}")
    print(f"\n{'warp':>10} {'mallas':>6} {'res':>8} {'ancho':>8} {'alto':>8} {'x0':>8} {'y0':>8} "
          f"{'w/bbox_kf':>9} {'h/bbox_kf':>9}")
    filas = []
    for i, casos in sorted(por_warp.items()):
        sols = np.array([c[0] for c in casos]).reshape(len(casos), 3, 2)
        sol = np.median(sols, axis=0)
        res = max(c[1] for c in casos)
        # rectángulo del dominio
        ancho = 5.0 / sol[0, 0] if sol[0, 0] else float("nan")
        alto = 5.0 / sol[1, 1] if sol[1, 1] else float("nan")
        x0 = -sol[2, 0] / sol[0, 0] if sol[0, 0] else float("nan")
        y0 = -sol[2, 1] / sol[1, 1] if sol[1, 1] else float("nan")
        # caja de la rejilla de reposo (keyform más regular)
        g = mod.rejillas[i][mod.kf_identidad(i)].reshape(-1, 2)
        bw, bh = g[:, 0].max() - g[:, 0].min(), g[:, 1].max() - g[:, 1].min()
        filas.append((i, len(casos), res, ancho, alto, x0, y0, ancho / bw, alto / bh))
        print(f"{mod.ids_def[i]:>10} {len(casos):6} {res:8.5f} {ancho:8.4f} {alto:8.4f} "
              f"{x0:8.4f} {y0:8.4f} {ancho / bw:9.3f} {alto / bh:9.3f}")

    if filas:
        r = np.array([[f[7], f[8]] for f in filas])
        print(f"\ncociente rect / caja de la rejilla de reposo: "
              f"mediana {np.median(r):.3f}  min {r.min():.3f}  max {r.max():.3f}")
        print(f"warps con el cociente en 1 ± 0.02: {int(((np.abs(r - 1) < 0.02).all(axis=1)).sum())}"
              f"/{len(filas)}")


if __name__ == "__main__":
    main()
