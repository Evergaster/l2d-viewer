"""Recalibra los rectángulos de dominio de los warps usando el DELTA entre poses.

El hallazgo de `docs/13` §5.15: calibrar con la pose de reposo deja pasar una ganancia
equivocada, porque en reposo el parámetro no se mueve. El objetivo correcto es

    modelo(AngleZ=10) − modelo(reposo)   ==   oráculo(AngleZ=10) − oráculo(reposo)

que cancela todo el offset estático y mide solo la ganancia.

Descenso por coordenadas sobre los 4 valores (x0, y0, ancho, alto) de cada warp, con una
penalización suave para no estropear la pose de reposo.

Uso: python3 tools/deform-delta-calibrar.py [vueltas]
"""
import importlib.util
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
ORA10 = "/home/evergaster/.hermes/cache/scratch/ora/z10.json"


class Delta:
    def __init__(self):
        d = MOD.leer_moc3(str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
        self.d = d
        self.mod = MOD.Modelo(d, json.load(open(RAIZ / "models/samples/hiyori.oracle.json")))
        self.bnd = MOD.Bindings(d)
        json.load(open(ORA10))  # comprobar que existe
        o0 = json.load(open(RAIZ / "models/samples/hiyori.oracle.json"))["drawables"]
        o10 = json.load(open(ORA10))["drawables"]
        self.o0 = {x["id"]: np.asarray(x["pos"], dtype=float).reshape(-1, 2) for x in o0}
        self.o10 = {x["id"]: np.asarray(x["pos"], dtype=float).reshape(-1, 2) for x in o10}
        nombres = json.load(open("/tmp/dominios-warp.json"))
        self.dom = {i: list(nombres[self.mod.ids_def[i]]) for i in range(self.mod.n_def)
                    if self.mod.tipos[i] == 0 and self.mod.ids_def[i] in nombres}
        self.warps = sorted(self.dom)
        # keyform por defecto (regla de Bindings) para reposo y para AngleZ=10
        self.kf2 = {}
        for i in range(self.mod.n_def):
            propio = (d["warp_deformer.keyform_binding_band_indices"] if self.mod.tipos[i] == 0
                      else d["rotation_deformer.keyform_binding_band_indices"])
            self.kf2[i] = self._indice(int(propio[int(self.mod.spec[i])]), int(self.mod.n_kf[i]), 10.0)
        self.kf1 = {}
        for i in range(self.mod.n_def):
            propio = (d["warp_deformer.keyform_binding_band_indices"] if self.mod.tipos[i] == 0
                      else d["rotation_deformer.keyform_binding_band_indices"])
            self.kf1[i] = self.bnd.indice(int(propio[int(self.mod.spec[i])]), int(self.mod.n_kf[i]))
        self.kfm = {m: self.bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                                       int(self.mod.kf_malla_cnt[m])) for m in range(self.mod.n_malla)}

    def _indice(self, banda, n_kf, anglez):
        lista = self.bnd.claves_de(banda)
        if not lista:
            return 0
        idx = 0
        for param, claves in lista:
            claves = np.asarray(claves, dtype=float)
            nombre = self.d["parameter.ids"][param] if param is not None else None
            v = anglez if nombre == "ParamAngleZ" else (float(self.bnd.defaults[param]) if param is not None else 0.0)
            if len(claves) <= 1:
                pos = 0
            else:
                pos = min(int(np.searchsorted(claves, v, side="left")), len(claves) - 1)
            idx = idx * len(claves) + pos
        return min(idx, max(n_kf - 1, 0))

    def evaluar(self, m, kfs):
        mod = self.mod
        p = mod.puntos_malla(m, self.kfm[m])
        for i in mod.cadenas[m]:
            k = kfs[i]
            if mod.tipos[i] == 1:
                s = int(mod.spec[i]); kk = int(mod.rot_kf_ini[s] + k)
                R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
                p = float(mod.rot_esc[kk]) * (p @ R.T) + np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
            else:
                x0, y0, w, h = self.dom[i]
                uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
                p = MOD.bilineal(mod.rejillas[i][k], uv)
        return p * np.array([1.0, -1.0])

    def errores(self):
        """(RMS del delta, mallas con delta < 1e-3, mallas de reposo < 1e-3)."""
        sumas, ok_delta, ok_repo = 0.0, 0, 0
        n = 0
        for m in range(self.mod.n_malla):
            nombre = self.mod.ids_malla[m]
            r0 = self.evaluar(m, self.kf1)
            r10 = self.evaluar(m, self.kf2)
            dm = r10 - r0
            do = self.o10[nombre] - self.o0[nombre]
            sumas += float(((dm - do) ** 2).sum())
            n += dm.size
            if float(np.abs(dm - do).max()) < 1e-3:
                ok_delta += 1
            if float(np.abs(r0 - self.o0[nombre]).max()) < 1e-3:
                ok_repo += 1
        return float(np.sqrt(sumas / n)), ok_delta, ok_repo


def main():
    vueltas = int(sys.argv[1]) if len(sys.argv) > 1 else 3
    cal = Delta()
    rms, okd, okr = cal.errores()
    print(f"inicio: RMS delta {rms:.6f}   delta<1e-3 {okd}/134   reposo<1e-3 {okr}/134")
    pasos = [0.02, 0.01, 0.005]
    for v in range(vueltas):
        cambios = 0
        for i in cal.warps:
            for idx in range(4):
                base = cal.dom[i][idx]
                mejor = (rms, base)
                for f in pasos:
                    for signo in (-1, 1):
                        cal.dom[i][idx] = base * (1 + signo * f)
                        if cal.dom[i][idx] <= 0:
                            continue
                        r = cal.errores()[0]
                        if r < mejor[0] - 1e-9:
                            mejor = (r, cal.dom[i][idx])
                cal.dom[i][idx] = mejor[1]
                if mejor[0] < rms:
                    rms = mejor[0]
                    cambios += 1
        rms, okd, okr = cal.errores()
        print(f"vuelta {v + 1}: RMS delta {rms:.6f}   delta<1e-3 {okd}/134   reposo<1e-3 {okr}/134  "
              f"({cambios} ajustes)")
        if cambios == 0:
            break
    json.dump({cal.mod.ids_def[i]: [round(x, 6) for x in cal.dom[i]] for i in cal.warps},
              open("/tmp/dominios-warp-delta.json", "w"), indent=1)
    print("dominios en /tmp/dominios-warp-delta.json")


if __name__ == "__main__":
    main()
