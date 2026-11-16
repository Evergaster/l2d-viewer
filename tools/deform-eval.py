"""Evaluador ÚNICO de una cadena, para no repetir la misma lógica en cada experimento.

Existe porque `docs/13` §5.13 dejó dos corridas del mismo caso que no coincidían: dos
implementaciones distintas de lo mismo. Aquí hay una sola: `evaluar()` devuelve la posición
final **y** los diagnósticos (vértices fuera del dominio por warp, uv mínimo y máximo).

Autocomprobación: en reposo, con la combinación conocida de ArtMesh63, tiene que dar ~6e-6
contra el oráculo. Si no da eso, el error está en este archivo y no en el modelo.

Uso:
  python3 tools/deform-eval.py                 # autocomprobación + barrido de AngleZ
  python3 tools/deform-eval.py --malla Otra    # otra malla
"""
import argparse
import importlib.util
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


class Evaluador:
    """Cadena de deformadores de un modelo, evaluada con la fórmula medida en docs/13."""

    def __init__(self, ruta_moc3, ruta_oraculo, ruta_dominios="/tmp/dominios-warp.json"):
        self.d = MOD.leer_moc3(str(ruta_moc3))
        self.mod = MOD.Modelo(self.d, json.load(open(ruta_oraculo)))
        self.bnd = MOD.Bindings(self.d)
        self.por_id = {x["id"]: np.asarray(x["pos"], dtype=float).reshape(-1, 2)
                       for x in json.load(open(ruta_oraculo))["drawables"]}
        nombres = json.load(open(ruta_dominios))
        self.dom = {i: tuple(nombres[self.mod.ids_def[i]]) for i in range(self.mod.n_def)
                    if self.mod.tipos[i] == 0 and self.mod.ids_def[i] in nombres}

    # ---- piezas ----
    def campos_rot(self, i, k):
        s = int(self.mod.spec[i])
        kk = int(self.mod.rot_kf_ini[s] + k)
        return (float(self.mod.rot_ang[kk]), float(self.mod.rot_ox[kk]),
                float(self.mod.rot_oy[kk]), float(self.mod.rot_esc[kk]))

    def rot(self, i, k, p, mezcla=None):
        """Rotación de un keyform. `mezcla=(k1, w)` extrapola/interpola hacia k1."""
        ang, ox, oy, esc = self.campos_rot(i, k)
        if mezcla is not None:
            k1, w = mezcla
            ang1, ox1, _, _ = self.campos_rot(i, k1)
            ang = ang + (ang1 - ang) * w
            ox = ox + (ox1 - ox) * w
        R = MOD.matriz_rot(ang + float(self.mod.rot_base[int(self.mod.spec[i])]))
        return esc * (p @ R.T) + np.array([ox, oy])

    def warp(self, i, k, p, registro=None):
        x0, y0, w, h = self.dom[i]
        uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
        if registro is not None:
            fuera = int(((uv < 0) | (uv > 5)).any(axis=1).sum())
            registro.append((self.mod.ids_def[i], k, fuera, len(uv),
                             float(uv.min()), float(uv.max())))
        return MOD.bilineal(self.mod.rejillas[i][k], uv)

    # ---- evaluación ----
    def evaluar(self, malla, kf_malla, combos, mezclas=None, diagnostico=False):
        """Devuelve (posiciones en espacio del Core, diagnóstico por warp)."""
        mod = self.mod
        m = mod.ids_malla.index(malla) if isinstance(malla, str) else malla
        p = mod.puntos_malla(m, kf_malla)
        registro = []
        for j, i in enumerate(mod.cadenas[m]):
            if mod.tipos[i] == 0:
                p = self.warp(i, combos[j], p, registro if diagnostico else None)
            else:
                mez = (mezclas or {}).get(mod.ids_def[i])
                p = self.rot(i, combos[j], p, mez)
        return p * np.array([1.0, -1.0]), registro

    def error(self, malla, kf_malla, combos, mezclas=None):
        p, _ = self.evaluar(malla, kf_malla, combos, mezclas)
        return float(np.abs(p - self.por_id[malla]).max())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--moc3", default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--oraculo-pose", default="/home/evergaster/.hermes/cache/scratch/ora/z10.json")
    ap.add_argument("--malla", default="ArtMesh63")
    ap.add_argument("--combo", default="4,1,1,0,1,1")
    args = ap.parse_args()

    ev = Evaluador(args.moc3, args.oraculo)
    combo = [int(x) for x in args.combo.split(",")]
    print(f"malla {args.malla}  cadena {[ev.mod.ids_def[i] for i in ev.mod.cadenas[ev.mod.ids_malla.index(args.malla)]]}")

    # 1) autocomprobación: reposo exacto
    e0 = ev.error(args.malla, 1, combo)
    print(f"reposo, combinación {tuple(combo)}: error {e0:.7f} "
          f"{'OK' if e0 < 1e-4 else 'REVISAR: el evaluador o el combo están mal'}")

    # 2) misma cadena en la pose de AngleZ=10, con y sin diagnostico
    ev10 = Evaluador(args.moc3, args.oraculo_pose)
    print(f"\ncontra AngleZ=10 (misma combinación): error {ev10.error(args.malla, 1, combo):.7f}")
    _, reg = ev10.evaluar(args.malla, 1, combo, diagnostico=True)
    for nombre, k, fuera, tot, mn, mx in reg:
        print(f"   {nombre:>8} kf={k}  {fuera:2}/{tot:3} fuera   uv [{mn:+.2f}, {mx:+.2f}]")

    # 3) barrido del tramo kf1->kf2 de la rotación que responde a AngleZ
    m = ev.mod.ids_malla.index(args.malla)
    cadena = ev.mod.cadenas[m]
    i_rot = None
    for i in cadena:
        b = int(ev.d["rotation_deformer.keyform_binding_band_indices"][int(ev.mod.spec[i])]) if ev.mod.tipos[i] == 1 else -1
        if b >= 0:
            claves = ev.bnd.claves_de(b)
            if claves and ev.d["parameter.ids"][claves[0][0]] == "ParamAngleZ":
                i_rot = ev.mod.ids_def[i]
    print(f"\nrotación que responde a ParamAngleZ: {i_rot}")
    if i_rot:
        print(f"{'peso':>6} {'error':>10} {'fuera (total)':>14}")
        for w in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5]:
            e = ev10.error(args.malla, 1, combo, {i_rot: (2, w)})
            _, reg = ev10.evaluar(args.malla, 1, combo, {i_rot: (2, w)}, diagnostico=True)
            fuera = sum(r[2] for r in reg)
            print(f"{w:6.2f} {e:10.6f} {fuera:14}")


if __name__ == "__main__":
    main()
