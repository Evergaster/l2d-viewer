"""Prueba la hipótesis de docs/13 §5.2: el Core **mezcla** dos keyforms vecinos.

Variantes medidas para la rotación, por eslabón:
  A  parámetros mezclados      ((1-w)·ang_j + w·ang_j+1, etc.)
  B  salidas mezcladas         ((1-w)·M_j(p) + w·M_j+1(p))          <- mezcla de matrices
  C  B con peso por valor normalizado del parámetro (min..max → 0..1)
  D  keyform más cercano al valor por defecto (sin mezcla)

Uso: python3 tools/rot-mezcla.py [modelo.moc3] [oraculo.json]
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


def cargar(ruta, oraculo):
    d = MOD.leer_moc3(ruta)
    mod = MOD.Modelo(d, json.load(open(oraculo)))
    por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
              for x in json.load(open(oraculo))["drawables"]}
    return d, mod, por_id


class Rot:
    """Rotación de un deformador, con sus keyforms y el peso del valor por defecto."""

    def __init__(self, d, mod, bnd, i):
        self.mod, self.i = mod, i
        self.s = int(mod.spec[i])
        self.n = int(mod.n_kf[i])
        self.banda = int(d["rotation_deformer.keyform_binding_band_indices"][self.s])
        lista = bnd.claves_de(self.banda)
        self.param, self.claves = lista[0] if lista else (None, np.array([0.0]))
        self.claves = np.asarray(self.claves, dtype=float)
        self.v = float(bnd.defaults[self.param]) if self.param is not None else 0.0
        # parámetro: min y max, para la variante C
        if self.param is not None:
            self.pmin = float(d["parameter.min_values"][self.param])
            self.pmax = float(d["parameter.max_values"][self.param])
        else:
            self.pmin, self.pmax = 0.0, 1.0

    def _campos(self, k):
        kk = int(self.mod.rot_kf_ini[self.s] + k)
        return (float(self.mod.rot_ang[kk]), float(self.mod.rot_ox[kk]),
                float(self.mod.rot_oy[kk]), float(self.mod.rot_esc[kk]))

    def par(self, p):
        """A: parámetros mezclados."""
        if len(self.claves) < 2:
            ang, ox, oy, esc = self._campos(0)
            w = 0.0
        elif self.v <= self.claves[0]:
            ang, ox, oy, esc = self._campos(0)
            w = 0.0
        elif self.v >= self.claves[-1]:
            ang, ox, oy, esc = self._campos(len(self.claves) - 1)
            w = 0.0
        else:
            j = int(np.searchsorted(self.claves, self.v) - 1)
            w = (self.v - self.claves[j]) / (self.claves[j + 1] - self.claves[j])
            a0 = self._campos(j)
            a1 = self._campos(j + 1)
            ang, ox, oy, esc = [a0[c] + (a1[c] - a0[c]) * w for c in range(4)]
        R = MOD.matriz_rot(ang + float(self.mod.rot_base[self.s]))
        return esc * (p @ R.T) + np.array([ox, oy])

    def _peso(self, modo):
        if len(self.claves) < 2:
            return 0, 0, 0.0
        if modo == "C":
            t = 0.0 if self.pmax == self.pmin else (self.v - self.pmin) / (self.pmax - self.pmin)
            t = min(max(t, 0.0), 1.0) * (len(self.claves) - 1)
            j = min(int(np.floor(t)), len(self.claves) - 2)
            return j, j + 1, float(t - j)
        if self.v <= self.claves[0]:
            return 0, 0, 0.0
        if self.v >= self.claves[-1]:
            return len(self.claves) - 1, len(self.claves) - 1, 0.0
        j = int(np.searchsorted(self.claves, self.v) - 1)
        w = (self.v - self.claves[j]) / (self.claves[j + 1] - self.claves[j])
        return j, j + 1, float(w)

    def una(self, k, p):
        ang, ox, oy, esc = self._campos(k)
        R = MOD.matriz_rot(ang + float(self.mod.rot_base[self.s]))
        return esc * (p @ R.T) + np.array([ox, oy])

    def salida(self, p, modo="B"):
        j0, j1, w = self._peso(modo)
        return (1.0 - w) * self.una(j0, p) + w * self.una(j1, p)

    def cercano(self, p):
        if len(self.claves) < 2:
            return self.una(0, p)
        k = int(np.argmin(np.abs(self.claves - self.v)))
        return self.una(k, p)


def main():
    ruta = sys.argv[1] if len(sys.argv) > 1 else str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3")
    oraculo = sys.argv[2] if len(sys.argv) > 2 else str(
        RAIZ / "models/samples/hiyori.oracle.json")

    d, mod, por_id = cargar(ruta, oraculo)
    bnd = MOD.Bindings(d)
    kf_malla = {m: bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                              int(mod.kf_malla_cnt[m])) for m in range(mod.n_malla)}
    rots = {i: Rot(d, mod, bnd, i) for i in range(mod.n_def) if mod.tipos[i] == 1}

    dominios = {}
    ruta_dom = Path("/tmp/dominios-warp.json")
    if ruta_dom.exists():
        nombres = json.load(open(ruta_dom))
        dominios = {i: tuple(nombres[mod.ids_def[i]]) for i in range(mod.n_def)
                    if mod.tipos[i] == 0 and mod.ids_def[i] in nombres}

    def evaluar(m, modo):
        p = mod.puntos_malla(m, kf_malla[m])
        for i in mod.cadenas[m]:
            if mod.tipos[i] == 1:
                r = rots[i]
                p = r.par(p) if modo == "A" else (r.cercano(p) if modo == "D" else r.salida(p, modo))
            elif i in dominios:
                x0, y0, w, h = dominios[i]
                uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
                p = MOD.bilineal(mod.rejillas[i][bnd.indice(
                    int(d["warp_deformer.keyform_binding_band_indices"][int(mod.spec[i])]),
                    int(mod.n_kf[i]))], uv)
            else:
                p = MOD.bilineal(mod.rejillas[i][bnd.indice(
                    int(d["warp_deformer.keyform_binding_band_indices"][int(mod.spec[i])]),
                    int(mod.n_kf[i]))], p * 5.0)
        return p * np.array([1.0, -1.0])

    print(f"{'modo':>6} {'<1e-3':>7} {'<6.7e-4':>9} {'mediana':>10} {'máx':>10}")
    for modo in ["D", "A", "B", "C"]:
        errs = np.array([float(np.abs(evaluar(m, modo) - por_id[mod.ids_malla[m]]).max())
                         for m in range(mod.n_malla)])
        print(f"{modo:>6} {int((errs < 1e-3).sum()):7} {int((errs < 6.7e-4).sum()):9} "
              f"{np.median(errs):10.5f} {errs.max():10.5f}")

    # detalle de las dos mallas problemáticas
    for nombre in ["ArtMesh102", "ArtMesh106"]:
        if nombre not in mod.ids_malla:
            continue
        m = mod.ids_malla.index(nombre)
        print(f"\n{nombre}: cadenas {[mod.ids_def[i] for i in mod.cadenas[m]]}")
        for i in mod.cadenas[m]:
            r = rots[i]
            j0, j1, w = r._peso("B")
            print(f"   {mod.ids_def[i]:>11} banda {r.banda:>2} param {r.param} claves {r.claves} "
                  f"def {r.v:.3f} -> kf {j0}/{j1} peso {w:.3f}")
        for modo in ["D", "A", "B", "C"]:
            err = float(np.abs(evaluar(m, modo) - por_id[nombre]).max())
            print(f"   modo {modo}: error {err:.6f}")


if __name__ == "__main__":
    main()
