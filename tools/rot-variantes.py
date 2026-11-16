"""Caza el error sistemático que queda en las rotaciones (docs/13 §5.1).

Para las dos mallas de sólo rotaciones cuya mejor combinación de keyform deja 0.0065, prueba
variantes de la fórmula de rotación por eslabón y busca la que llega a cero.

Uso: python3 tools/rot-variantes.py [modelo.moc3] [oraculo.json]
"""
import importlib.util
import itertools
import json
import sys
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parent.parent


def modulo():
    spec = importlib.util.spec_from_file_location("deform_buscar", RAIZ / "tools" / "deform-buscar.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MOD = modulo()

MALLAS = ["ArtMesh102", "ArtMesh106"]
# keyform por eslabón medidos como mejores en docs/13 §5.1
MEJORES = {
    "ArtMesh102": [1, 1, 2],
    "ArtMesh106": [1, 1, 1, 0],
}


def rot(mod, i, k, p, variante):
    s = int(mod.spec[i])
    kk = int(mod.rot_kf_ini[s] + k)
    esc = float(mod.rot_esc[kk])
    th = float(mod.rot_ang[kk]) + float(mod.rot_base[s])
    if variante in ("angulo_negado", "negado_pivote"):
        th = -th
    R = MOD.matriz_rot(th)
    O = np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
    rx = -1.0 if (mod.reflect_x.size and mod.reflect_x[kk]) else 1.0
    ry = -1.0 if (mod.reflect_y.size and mod.reflect_y[kk]) else 1.0

    if variante in ("actual", "angulo_negado"):
        q = p * np.array([rx, ry])
        return esc * (q @ R.T) + O
    if variante in ("pivote", "negado_pivote"):
        q = (p - O) * np.array([rx, ry])
        return esc * (q @ R.T) + O
    if variante == "espejo_despues":
        q = esc * (p @ R.T)
        return q * np.array([rx, ry]) + O
    if variante == "escala_en_origen":
        q = p * np.array([rx, ry])
        return esc * (q @ R.T) + esc * O
    if variante == "origen_espejado":
        q = p * np.array([rx, ry])
        return esc * (q @ R.T) + O * np.array([rx, ry])
    if variante == "sin_espejo":
        return esc * (p @ R.T) + O
    raise ValueError(variante)


VARIANTES = ["actual", "pivote", "angulo_negado", "negado_pivote", "espejo_despues",
             "escala_en_origen", "origen_espejado", "sin_espejo"]


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
    kf_malla = {m: bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                              int(mod.kf_malla_cnt[m])) for m in range(mod.n_malla)}

    for nombre in MALLAS:
        if nombre not in mod.ids_malla:
            print(f"{nombre}: no está en este modelo")
            continue
        m = mod.ids_malla.index(nombre)
        cadena = mod.cadenas[m]
        obj = por_id[nombre]
        print(f"\n{nombre}: cadena {[mod.ids_def[i] for i in cadena]}")
        # reflect de los keyforms usados
        detalles = []
        for i, k in zip(cadena, MEJORES[nombre]):
            s = int(mod.spec[i])
            kk = int(mod.rot_kf_ini[s] + k)
            det = dict(ang=float(mod.rot_ang[kk]), base=float(mod.rot_base[s]),
                       esc=float(mod.rot_esc[kk]), ox=float(mod.rot_ox[kk]), oy=float(mod.rot_oy[kk]),
                       rx=int(mod.reflect_x[kk]) if mod.reflect_x.size else None,
                       ry=int(mod.reflect_y[kk]) if mod.reflect_y.size else None)
            detalles.append(det)
            print(f"   {mod.ids_def[i]:>11} kf{k}: {det}")

        # 1) una variante para toda la cadena
        print("   variante única en la cadena:")
        for v in VARIANTES:
            p = mod.puntos_malla(m, kf_malla[m])
            for i, k in zip(cadena, MEJORES[nombre]):
                p = rot(mod, i, k, p, v)
            err = float(np.abs(p * np.array([1, -1]) - obj).max())
            print(f"     {v:>16}: {err:.6f}")

        # 2) variante por eslabón (todas las combinaciones)
        print("   variante por eslabón (mejores combinaciones):")
        resultados = []
        for combo in itertools.product(VARIANTES, repeat=len(cadena)):
            p = mod.puntos_malla(m, kf_malla[m])
            for i, k, v in zip(cadena, MEJORES[nombre], combo):
                p = rot(mod, i, k, p, v)
            err = float(np.abs(p * np.array([1, -1]) - obj).max())
            resultados.append((err, combo))
        resultados.sort()
        for err, combo in resultados[:5]:
            print(f"     {err:.6f}  {combo}")


if __name__ == "__main__":
    main()
