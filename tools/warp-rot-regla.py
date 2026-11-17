"""La pieza que faltaba, según la especificación no oficial: **rotación hija de un warp**.

Fuente (independiente y verificable, no el SDK): `AyagamiDev/docs · model/moc3_file_format.md`
§ «Deformer chaining»:

> The last case, **a rotation deformer that is a child of a warp deformer, is special**:
> - To deform the scale value, walk up the deformer tree until a rotation deformer is found,
>   then apply its scale value (multiplicatively). […] intervening warp deformers are ignored.
> - To deform the translation vector, take that vector as a vertex position and apply the
>   parent deformer's warp transformation to it.
> - To deform the angle, take the translation vector, subtract 0.1 in the Y direction, and
>   apply the parent deformer's warp transformation to it. The rotation of the resulting
>   vector […] becomes an offset added to the angle.

Eso explica los tres síntomas medidos en `docs/13`: que la escala del eslabón «aplaste» la
cadena (§5.24: `Rotation2` con `escala = 1/ppu` aplicada a coordenadas ya normalizadas), que el
origen quede desplazado (§5.25: el ajuste dio justo el `origin` del keyform) y que la ganancia
frente al parámetro salga a la mitad (§5.15).

Este script mide la regla contra el oráculo, sin tocar el código del producto hasta ver números.

Uso:
  python3 tools/warp-rot-regla.py                # Hiyori
  python3 tools/warp-rot-regla.py --probe -0.1   # signo del sondeo del ángulo
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
    ap.add_argument("--modelo",
                    default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--probe", type=float, default=-0.1,
                    help="delta en Y para el sondeo del ángulo (la spec dice restar 0.1)")
    ap.add_argument("--sin-regla", action="store_true", help="referencia: sin la regla")
    ap.add_argument("--solo", default="escala,origen,angulo",
                    help="sub-reglas activas: escala,origen,angulo")
    args = ap.parse_args()

    d = MOD.leer_moc3(args.modelo)
    mod = MOD.Modelo(d, json.load(open(args.oraculo)))
    bnd = MOD.Bindings(d)

    kf_def, kf_malla = {}, {}
    for i in range(mod.n_def):
        propia = (d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                  else d["rotation_deformer.keyform_binding_band_indices"])
        kf_def[i] = bnd.indice(int(propia[int(mod.spec[i])]), int(mod.n_kf[i]))
    for m in range(mod.n_malla):
        kf_malla[m] = bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                                 int(mod.kf_malla_cnt[m]))

    def warp(i, k, p):
        return MOD.bilineal(mod.rejillas[i][k], p * 5.0)

    def campos(i, k):
        s = int(mod.spec[i])
        kk = int(mod.rot_kf_ini[s] + k)
        return (float(mod.rot_ang[kk]), np.array([mod.rot_ox[kk], mod.rot_oy[kk]]),
                float(mod.rot_esc[kk]), kk)

    def escala_ancestro(i):
        """escala de la rotación más cercana subiendo el árbol (None si no hay)."""
        p = int(mod.padre[i])
        while p != -1:
            if mod.tipos[p] == 1:
                return campos(p, kf_def[p])[2]
            p = int(mod.padre[p])
        return None

    def rot_efectiva(i, k, p):
        ang, origen, esc, kk = campos(i, k)
        base = float(mod.rot_base[int(mod.spec[i])])
        padre = int(mod.padre[i])
        activas = set(x.strip() for x in args.solo.split(",") if x.strip())
        if not args.sin_regla and padre != -1 and mod.tipos[padre] == 0:
            hacer = activas
            # 1) traslación: el origen es un vértice del warp padre
            if "origen" in hacer:
                origen = warp(padre, kf_def[padre], origen[None, :])[0]
            # 2) escala: se multiplica por la de la rotación ancestro más cercana
            anc = escala_ancestro(i)
            if anc and "escala" in hacer:
                esc = esc * anc
            # 3) ángulo: se sondea el warp con un desplazamiento en Y
            a = origen
            b = warp(padre, kf_def[padre], (origen + np.array([0.0, args.probe]))[None, :])[0]
            v = b - a
            if "angulo" in hacer and np.linalg.norm(v) > 1e-12:
                ang += np.degrees(np.arctan2(v[1], v[0])) - np.degrees(
                    np.arctan2(args.probe, 0.0))
        if mod.reflect_x.size:
            p = p * np.array([-1.0 if mod.reflect_x[kk] else 1.0,
                              -1.0 if mod.reflect_y[kk] else 1.0])
        # especificación: escalar → rotar → trasladar
        R = MOD.matriz_rot(ang + base)
        return esc * (p @ R.T) + origen

    def evaluar(m):
        p = mod.puntos_malla(m, kf_malla[m])
        for i in mod.cadenas[m]:            # hoja -> raíz
            k = kf_def[i]
            p = warp(i, k, p) if mod.tipos[i] == 0 else rot_efectiva(i, k, p)
        return p * np.array([1.0, -1.0])

    errs = []
    for m in range(mod.n_malla):
        o = mod.oraculo[mod.ids_malla[m]]
        p = evaluar(m)
        if len(o) == len(p):
            errs.append((mod.ids_malla[m], len(mod.cadenas[m]), float(np.abs(p - o).max())))
    errs.sort(key=lambda x: x[2])
    exactas = sum(1 for e in errs if e[2] < 1e-3)
    dos_px = sum(1 for e in errs if e[2] < 6.7e-4)
    print(f"{'sin regla' if args.sin_regla else 'con la regla de la spec'}  probe={args.probe:+.2f}: "
          f"< 1e-3 en {exactas}/{len(errs)}   < 2 px: {dos_px}   "
          f"mediana {np.median([e[2] for e in errs]):.2e}   máximo {errs[-1][2]:.4f}")
    print("  10 mejores:")
    for nombre, n, e in errs[:10]:
        print(f"    {nombre:<12} eslabones={n:<2} error={e:.6f}")
    print("  5 peores:")
    for nombre, n, e in errs[-5:]:
        print(f"    {nombre:<12} eslabones={n:<2} error={e:.6f}")


if __name__ == "__main__":
    main()
