"""¿En qué espacio está la entrada de cada warp: modelo (isotrópico) o lienzo normalizado?

Sale de un dato de `docs/13` §5.26: el rectángulo de dominio calibrado para `Warp1` mide
0,569 × 0,798, y 0,798 / 0,569 = **1,4032**, que es exactamente `alto / ancho` del lienzo
(4175 / 2976 = 1,4029). Es decir: ese dominio **es un cuadrado en coordenadas normalizadas
del lienzo** (`x/W`, `y/H`), no en unidades del modelo.

De ahí la hipótesis, que se mide aquí: la entrada de cada warp se normaliza o bien por el
ancho (`u = 5·x`, `v = 5·y`, lo de siempre) o bien por el lienzo (`u = 5·x`, `v = 5·y·W/H`).
Cuál es, lo dice la propia jaula: una jaula dibujada «cuadrada» en el editor de Cubism sale
con `alto/ancho ≈ 1` si el espacio es el modelo y `≈ H/W` si es el lienzo.

Uso:
  python3 tools/deform-espacio.py
  python3 tools/deform-espacio.py --modelo …/Wanko.moc3 --oraculo models/samples/wanko.oracle.json
"""
import argparse
import importlib.util
import json
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parent.parent


def modulo(nombre, archivo):
    spec = importlib.util.spec_from_file_location(nombre, RAIZ / "tools" / archivo)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MOD = modulo("deform_buscar", "deform-buscar.py")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--modelo", default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    args = ap.parse_args()

    d = MOD.leer_moc3(args.modelo)
    mod = MOD.Modelo(d, json.load(open(args.oraculo)))
    bnd = MOD.Bindings(d)

    ancho_lienzo = float(d["canvas"][0]) if isinstance(d.get("canvas"), (list, tuple)) else None
    # el lienzo viene en el volcado como cabecera aparte; se calcula de la sección si no está
    ppu = float(d.get("__ppu", 2976.0))
    del ancho_lienzo

    # alto/ancho del lienzo: se despeja de la cabecera del moc3 si está, si no 4175/2976
    aspecto = 4175.0 / 2976.0
    del ppu

    # clasificación por jaula
    def aspecto_jaula(i, k=0):
        g = mod.rejillas[i][k].reshape(-1, 2)
        w = float(g[:, 0].max() - g[:, 0].min())
        h = float(g[:, 1].max() - g[:, 1].min())
        return h / w if w else 0.0

    clases = {}
    for i in range(mod.n_def):
        if mod.tipos[i] != 0:
            continue
        r = aspecto_jaula(i)
        # ¿más cerca de cuadrada en el modelo (1) o en el lienzo (aspecto)?
        clases[i] = "lienzo" if abs(r - aspecto) < abs(r - 1.0) else "modelo"
    n_lienzo = sum(1 for v in clases.values() if v == "lienzo")
    print(f"jaulas: {n_lienzo} en lienzo normalizado, {len(clases) - n_lienzo} en modelo "
          f"(aspecto del lienzo {aspecto:.4f})")

    # evaluación con el espacio elegido por jaula
    def warpar(i, k, p):
        rejilla = mod.rejillas[i][k]
        uv = p * 5.0
        if clases[i] == "lienzo":
            uv = uv * np.array([1.0, 1.0 / aspecto])   # y normalizada por la ALTURA del lienzo
        return MOD.bilineal(rejilla, uv)

    def evaluar(m, kf_malla, combos):
        p = mod.puntos_malla(m, kf_malla)
        for j, i in enumerate(mod.cadenas[m]):
            if mod.tipos[i] == 0:
                p = warpar(i, combos[j], p)
            else:
                s = int(mod.spec[i])
                kk = int(mod.rot_kf_ini[s] + combos[j])
                esc = float(mod.rot_esc[kk])
                R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
                if mod.reflect_x.size:
                    p = p * np.array([-1.0 if mod.reflect_x[kk] else 1.0,
                                      -1.0 if mod.reflect_y[kk] else 1.0])
                p = esc * (p @ R.T) + np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
        return p * np.array([1.0, -1.0])

    errores = []
    for m in range(mod.n_malla):
        nombre = mod.ids_malla[m]
        kf_m = bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]), int(mod.kf_malla_cnt[m]))
        combos = [bnd.indice(int(d["deformer.keyform_binding_band_indices"][i]), int(mod.n_kf[i]))
                  for i in mod.cadenas[m]]
        p = evaluar(m, kf_m, combos)
        o = mod.oraculo[nombre]
        if len(o) != len(p):
            continue
        errores.append((nombre, len(mod.cadenas[m]),
                        float(np.abs(p - o).max())))
    errores.sort(key=lambda x: x[2])
    exactas = [e for e in errores if e[2] < 1e-3]
    print(f"\ncon el espacio elegido por jaula: < 1e-3 en {len(exactas)}/{len(errores)}"
          f"  mediana {np.median([e[2] for e in errores]):.4f}"
          f"  máximo {errores[-1][2]:.4f}")
    print("  (referencia con todo en espacio modelo: 12/134, mediana 0,0327)")
    print("\n  mejores:")
    for nombre, n, e in errores[:10]:
        print(f"    {nombre:<12} eslabones={n:<2} error={e:.6f}")


if __name__ == "__main__":
    main()
