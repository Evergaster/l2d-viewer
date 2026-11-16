"""Mide la selección de keyform de cada malla contra un oráculo cualquiera.

Para cada malla recorre TODAS las combinaciones de keyform de su cadena, se queda con la que
mejor reproduce el oráculo y la compara con la que predice la regla. Sirve para cualquier pose:

  1. Generar el oráculo con `models/samples/dump.html` servido por HTTP y el override `?p=`
     (por ejemplo `?m=Hiyori&p=ParamAngleZ=10`) y guardar `window.__dump` en un archivo.
  2. `python3 tools/keyform-medir.py <modelo.moc3> <oraculo.json> [--max-largo N]`

Salida: por malla, la combinación de la regla, la mejor combinación y el error de cada una.
"""
import argparse
import importlib.util
import itertools
import json
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parent.parent


def modulo():
    spec = importlib.util.spec_from_file_location("deform_buscar", RAIZ / "tools" / "deform-buscar.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MOD = modulo()


def main():
    ap = argparse.ArgumentParser(description="Selección de keyform medida contra un oráculo")
    ap.add_argument("modelo", nargs="?", default=str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("oraculo", nargs="?", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--max-largo", type=int, default=4,
                    help="sólo cadenas de hasta N deformadores (la búsqueda es exponencial)")
    ap.add_argument("--umbral", type=float, default=1e-3)
    ap.add_argument("--pose", default="",
                    help="valores de parámetro de la pose del oráculo: ParamAngleZ=10,ParamEyeBallX=1")
    args = ap.parse_args()

    pose = {}
    for trozo in filter(None, (t.strip() for t in args.pose.split(","))):
        nombre, _, valor = trozo.partition("=")
        pose[nombre.strip()] = float(valor)

    d = MOD.leer_moc3(args.modelo)
    mod = MOD.Modelo(d, json.load(open(args.oraculo)))
    bnd = MOD.Bindings(d)
    por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
              for x in json.load(open(args.oraculo))["drawables"]}
    kf_malla = {m: bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                              int(mod.kf_malla_cnt[m])) for m in range(mod.n_malla)}
    dominios = {}
    if Path("/tmp/dominios-warp.json").exists():
        nombres = json.load(open("/tmp/dominios-warp.json"))
        dominios = {i: tuple(nombres[mod.ids_def[i]]) for i in range(mod.n_def)
                    if mod.tipos[i] == 0 and mod.ids_def[i] in nombres}

    def aplicar(i, k, p):
        if mod.tipos[i] == 1:
            s = int(mod.spec[i])
            kk = int(mod.rot_kf_ini[s] + k)
            R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
            return float(mod.rot_esc[kk]) * (p @ R.T) + np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
        rej = mod.rejillas[i][k]
        if i in dominios:
            x0, y0, w, h = dominios[i]
            uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
        else:
            uv = p * 5.0
        return MOD.bilineal(rej, uv)

    regla_ok = medidas = 0

    def indice_banda(banda, n_kf):
        """Índice de keyform por radix mixto (primer parámetro el más significativo)."""
        lista = bnd.claves_de(banda)
        if not lista:
            return 0
        idx = 0
        for param, claves in lista:
            claves = np.asarray(claves, dtype=float)
            nombre = d["parameter.ids"][param] if param is not None else None
            if nombre in pose:
                v = pose[nombre]
            else:
                v = float(bnd.defaults[param]) if param is not None else 0.0
            if len(claves) <= 1:
                pos = 0
            elif v <= claves[0]:
                pos = 0
            elif v >= claves[-1]:
                pos = len(claves) - 1
            else:
                pos = int(np.searchsorted(claves, v) - 1)
            idx = idx * len(claves) + pos
        return min(idx, max(n_kf - 1, 0))

    print(f"{'malla':>12} {'n':>2} {'regla':>12} {'mejor':>12} {'err_regla':>10} {'err_mejor':>10}")
    for m in range(mod.n_malla):
        cadena = mod.cadenas[m]
        if len(cadena) > args.max_largo or any(i not in dominios and mod.tipos[i] == 0 for i in cadena):
            continue
        nombre = mod.ids_malla[m]
        obj = por_id[nombre]
        regla = []
        for i in cadena:
            propio = (d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                      else d["rotation_deformer.keyform_binding_band_indices"])
            regla.append(indice_banda(int(propio[int(mod.spec[i])]), int(mod.n_kf[i])))
        p0 = mod.puntos_malla(m, kf_malla[m])

        def err(combo):
            p = p0.copy()
            for i, k in zip(cadena, combo):
                p = aplicar(i, k, p)
            return float(np.abs(p * np.array([1.0, -1.0]) - obj).max())

        mejor = min((err(c), c) for c in itertools.product(*[range(mod.n_kf[i]) for i in cadena]))
        e_regla = err(tuple(regla))
        medidas += 1
        acierto = tuple(regla) == mejor[1]
        regla_ok += int(acierto)
        if not acierto or e_regla > args.umbral:
            print(f"{nombre:>12} {len(cadena):2} {str(tuple(regla)):>12} {str(mejor[1]):>12} "
                  f"{e_regla:10.5f} {mejor[0]:10.5f}")

    print(f"\nmedidas: {medidas}   la regla coincide con la mejor combinación en {regla_ok}")


if __name__ == "__main__":
    main()
