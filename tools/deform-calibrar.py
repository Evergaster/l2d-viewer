"""Calibra el rectángulo de dominio de TODOS los warps por descenso por bloques.

Modelo: un warp aplica `out = bilineal(rejilla_elegida, (p − origen) / tamaño · 5)`, donde el
rectángulo (origen, tamaño) es el dato que el archivo no guarda explícito. Aquí no se adivina:
se resuelve numéricamente contra el oráculo del Core, warp por warp, repitiendo hasta que el
error de las 134 mallas deja de bajar. El residuo final dice si el modelo es correcto.

Uso: python3 tools/deform-calibrar.py [modelo.moc3] [oraculo.json] [rondas]
Salida: /tmp/dominios-warp.json con los rectángulos medidos.
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


def invertir_rejilla(rej, pts, iteraciones=60):
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


class Entorno:
    def __init__(self, ruta, oraculo):
        self.d = MOD.leer_moc3(ruta)
        self.mod = MOD.Modelo(self.d, json.load(open(oraculo)))
        self.oraculo = json.load(open(oraculo))
        self.por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
                       for x in self.oraculo["drawables"]}
        bnd = MOD.Bindings(self.d)
        self.kf_def = {}
        for i in range(self.mod.n_def):
            propio = (self.d["warp_deformer.keyform_binding_band_indices"] if self.mod.tipos[i] == 0
                      else self.d["rotation_deformer.keyform_binding_band_indices"])
            self.kf_def[i] = bnd.indice(int(propio[int(self.mod.spec[i])]), int(self.mod.n_kf[i]))
        self.kf_malla = {m: bnd.indice(int(self.d["art_mesh.keyform_binding_band_indices"][m]),
                                       int(self.mod.kf_malla_cnt[m]))
                         for m in range(self.mod.n_malla)}

    def rot_fwd(self, i, p):
        md, k = self.mod, self.kf_def[i]
        s = int(md.spec[i])
        kk = int(md.rot_kf_ini[s] + k)
        R = MOD.matriz_rot(float(md.rot_ang[kk]) + float(md.rot_base[s]))
        if md.reflect_x.size:
            p = p * np.array([-1.0 if md.reflect_x[kk] else 1.0, -1.0 if md.reflect_y[kk] else 1.0])
        return float(md.rot_esc[kk]) * (p @ R.T) + np.array([md.rot_ox[kk], md.rot_oy[kk]])

    def rot_inv(self, i, p):
        md, k = self.mod, self.kf_def[i]
        s = int(md.spec[i])
        kk = int(md.rot_kf_ini[s] + k)
        R = MOD.matriz_rot(float(md.rot_ang[kk]) + float(md.rot_base[s]))
        q = ((p - np.array([md.rot_ox[kk], md.rot_oy[kk]])) / float(md.rot_esc[kk])) @ R
        if md.reflect_x.size:
            q = q * np.array([-1.0 if md.reflect_x[kk] else 1.0, -1.0 if md.reflect_y[kk] else 1.0])
        return q

    def aplicar(self, i, p, dom):
        if self.mod.tipos[i] != 0:
            return self.rot_fwd(i, p)
        x0, y0, w, h = dom[i]
        uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
        return MOD.bilineal(self.mod.rejillas[i][self.kf_def[i]], uv)

    def invertir(self, i, p, dom):
        if self.mod.tipos[i] != 0:
            return self.rot_inv(i, p)
        x0, y0, w, h = dom[i]
        uv = invertir_rejilla(self.mod.rejillas[i][self.kf_def[i]], p)
        return np.array([x0, y0]) + uv / 5.0 * np.array([w, h])

    def evaluar(self, m, dom):
        p = self.mod.puntos_malla(m, self.kf_malla[m])
        for i in self.mod.cadenas[m]:
            p = self.aplicar(i, p, dom)
        return p * np.array([1.0, -1.0])

    def error_global(self, dom):
        errs = [float(np.abs(self.evaluar(m, dom) - self.por_id[self.mod.ids_malla[m]]).max())
                for m in range(self.mod.n_malla)]
        return np.array(errs)


def main():
    ruta = sys.argv[1] if len(sys.argv) > 1 else str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3")
    oraculo = sys.argv[2] if len(sys.argv) > 2 else str(
        RAIZ / "models/samples/hiyori.oracle.json")
    rondas = int(sys.argv[3]) if len(sys.argv) > 3 else 6

    e = Entorno(ruta, oraculo)
    mod = e.mod
    warps = [i for i in range(mod.n_def) if mod.tipos[i] == 0]
    dom = {i: [0.0, 0.0, 1.0, 1.0] for i in warps}      # punto de partida: cuadrado unidad

    err = e.error_global(dom)
    print(f"inicio (todos en cuadrado unidad): {sum(1 for x in err if x < 1e-3)}/{len(err)} "
          f"< 1e-3, mediana {np.median(err):.4f}")

    casos = defaultdict(list)
    for m in range(mod.n_malla):
        for j, i in enumerate(mod.cadenas[m]):
            if mod.tipos[i] == 0:
                casos[i].append((m, j))

    for r in range(rondas):
        for i in warps:
            nuevos = []
            for m, j in casos[i]:
                p = mod.puntos_malla(m, e.kf_malla[m])
                for x in mod.cadenas[m][:j]:
                    p = e.aplicar(x, p, dom)
                a = p
                b = e.por_id[mod.ids_malla[m]] * np.array([1.0, -1.0])
                for x in reversed(mod.cadenas[m][j + 1:]):
                    b = e.invertir(x, b, dom)
                uv = invertir_rejilla(mod.rejillas[i][e.kf_def[i]], b)
                A = np.hstack([a, np.ones((len(a), 1))])
                sol, *_ = np.linalg.lstsq(A, uv, rcond=None)
                if abs(sol[0, 0]) < 1e-9 or abs(sol[1, 1]) < 1e-9:
                    continue
                res = float(np.abs(A @ sol - uv).max())
                if res > 5e-2:
                    continue
                nuevos.append((-sol[2, 0] / sol[0, 0], -sol[2, 1] / sol[1, 1],
                               5.0 / sol[0, 0], 5.0 / sol[1, 1]))
            if nuevos:
                dom[i] = list(np.median(np.array(nuevos), axis=0))
        err = e.error_global(dom)
        print(f"ronda {r + 1}: {sum(1 for x in err if x < 1e-3)}/{len(err)} < 1e-3, "
              f"mediana {np.median(err):.5f}, máx {err.max():.4f}")

    print(f"\nwarps calibrados: {len(warps)}")
    filas = []
    for i in warps:
        g = mod.rejillas[i][mod.kf_identidad(i)].reshape(-1, 2)
        bw, bh = g[:, 0].max() - g[:, 0].min(), g[:, 1].max() - g[:, 1].min()
        d = dom[i]
        filas.append((mod.ids_def[i], d, d[2] / bw if bw else float("nan"), d[3] / bh if bh else float("nan")))
    for nombre, d, rx, ry in filas[:12]:
        print(f"  {nombre:>10} x0={d[0]:+.4f} y0={d[1]:+.4f} w={d[2]:.4f} h={d[3]:.4f}  "
              f"w/bbox={rx:.3f} h/bbox={ry:.3f}")
    r = np.array([[f[2], f[3]] for f in filas if np.isfinite(f[2])])
    print(f"\ncociente dominio/caja rejilla de reposo: mediana {np.median(r):.4f} "
          f"min {r.min():.4f} max {r.max():.4f}")
    json.dump({f[0]: [round(x, 6) for x in f[1]] for f in filas},
              open("/tmp/dominios-warp.json", "w"), indent=1)
    print("dominios en /tmp/dominios-warp.json")


if __name__ == "__main__":
    main()
