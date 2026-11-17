"""Hipótesis FFD: el warp normaliza su entrada invirtiendo **su rejilla neutra**.

Por qué existe este script (`docs/13` §5.21 dejó la pregunta abierta):

- La especificación dice que la entrada de un warp es `[0,1]²` uniforme, pero con ese
  dominio el reposo no cuadra (12/134) y la calibración acabó inventando rectángulos
  (`Warp1` = 0,569×0,798) que no vienen de ningún campo del archivo.
- Lo que sí cuadra es un dominio rectangular por warp, y §5.25 midió que el desfase por
  eje es exactamente el recíproco de los lados de ese rectángulo: señal de que el
  rectángulo normaliza una **rejilla que no es el cuadrado unidad**.
- La lectura natural de una jaula (free-form deformation) es la que nunca se probó con
  la cadena entera construida así: el warp no adivina el dominio, lo **invierte de su
  propia rejilla neutra** — el keyform de la pose por defecto, la forma original de la
  jaula. Si esa rejilla *es* el cuadrado unidad, la hipótesis coincide con `u = 5p` y
  explica por qué las cadenas cortas salían exactas; si no lo es, es la pieza que falta.

Uso:
  python3 tools/deform-ffd.py                  # jaulas + reposo + delta con AngleZ
  python3 tools/deform-ffd.py --todos          # lista de las 134 mallas
  python3 tools/deform-ffd.py --neutro cero    # otra convención de «rejilla neutra»
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


# --------------------------------------------------------------------------- #
# inversión de la jaula
# --------------------------------------------------------------------------- #
def invertir(rejilla, p, iteraciones=40):
    """`(uv, dentro)` con `bilineal(rejilla, uv) = p`, celda por celda por Newton.

    `uv` en coordenadas de celda `[0,5]`. `dentro` marca los puntos que cayeron en
    alguna celda; los de fuera se quedan con la mejor solución (el Core también
    extrapola), pero se cuentan aparte.
    """
    filas, cols, _ = rejilla.shape
    n = p.shape[0]
    mejor_uv = np.full((n, 2), np.nan)
    mejor_err = np.full(n, np.inf)
    dentro = np.zeros(n, dtype=bool)

    for j in range(filas - 1):
        for i in range(cols - 1):
            P00, P10 = rejilla[j, i], rejilla[j, i + 1]
            P01, P11 = rejilla[j + 1, i], rejilla[j + 1, i + 1]
            uv = np.tile(np.array([i + 0.5, j + 0.5]), (n, 1))

            for _ in range(iteraciones):
                fu = uv[:, 0:1] - i
                fv = uv[:, 1:2] - j
                B = ((1 - fu) * (1 - fv) * P00 + fu * (1 - fv) * P10
                     + (1 - fu) * fv * P01 + fu * fv * P11)
                Du = (1 - fv) * (P10 - P00) + fv * (P11 - P01)
                Dv = (1 - fu) * (P01 - P00) + fu * (P11 - P10)
                R = p - B
                det = Du[:, 0] * Dv[:, 1] - Du[:, 1] * Dv[:, 0]
                det = np.where(np.abs(det) < 1e-18, 1e-18, det)
                uv = uv + np.stack(
                    [(R[:, 0] * Dv[:, 1] - R[:, 1] * Dv[:, 0]) / det,
                     (-R[:, 0] * Du[:, 1] + R[:, 1] * Du[:, 0]) / det], axis=1)

            fu = uv[:, 0:1] - i
            fv = uv[:, 1:2] - j
            B = ((1 - fu) * (1 - fv) * P00 + fu * (1 - fv) * P10
                 + (1 - fu) * fv * P01 + fu * fv * P11)
            err = np.linalg.norm(p - B, axis=1)
            en_celda = ((fu[:, 0] >= -1e-9) & (fu[:, 0] <= 1 + 1e-9)
                        & (fv[:, 0] >= -1e-9) & (fv[:, 0] <= 1 + 1e-9))
            mejor = ((en_celda & ~dentro)
                     | (en_celda & dentro & (err < mejor_err))
                     | (~en_celda & ~dentro & (err < mejor_err)))
            mejor_uv[mejor] = uv[mejor]
            mejor_err[mejor] = err[mejor]
            dentro |= (en_celda & mejor)

    return mejor_uv, dentro


# --------------------------------------------------------------------------- #
# el modelo con la hipótesis FFD
# --------------------------------------------------------------------------- #
class FFD:
    def __init__(self, ruta_moc3, ruta_oraculo, neutro="regla"):
        self.d = MOD.leer_moc3(str(ruta_moc3))
        self.mod = MOD.Modelo(self.d, json.load(open(ruta_oraculo)))
        self.bnd = MOD.Bindings(self.d)
        self.neutro = neutro
        self.oraculo = self._oraculo(ruta_oraculo)
        self.kf_neutro = {}
        for i in range(self.mod.n_def):
            if self.mod.tipos[i] != 0:
                continue
            s = int(self.mod.spec[i])
            banda = int(self.d["warp_deformer.keyform_binding_band_indices"][s])
            n_kf = int(self.mod.w_kf_cnt[s])
            if neutro == "regla":
                k = int(self.bnd.indice(banda, n_kf))
            elif neutro == "cero":
                k = 0
            else:
                k = self.mod.kf_identidad(i)
            self.kf_neutro[i] = k

    @staticmethod
    def _oraculo(ruta):
        return {x["id"]: np.asarray(x["pos"], dtype=float).reshape(-1, 2)
                for x in json.load(open(ruta))["drawables"]}

    def pose(self, ruta):
        """Cambia el oráculo de referencia (otra pose) sin rehacer el modelo."""
        self.oraculo = self._oraculo(ruta)

    # -- diagnóstico de las jaulas ---------------------------------------- #
    def forma_de_la_jaula(self, i):
        rej = self.mod.rejillas[i][self.kf_neutro[i]]
        filas, cols, _ = rej.shape
        plano = rej.reshape(-1, 2)
        mn = plano.min(0)
        mx = plano.max(0)
        u = np.linspace(0, 1, cols)[None, :]
        v = np.linspace(0, 1, filas)[:, None]
        unidad = np.stack([np.broadcast_to(u, (filas, cols)),
                           np.broadcast_to(v, (filas, cols))], axis=2)
        return {
            "bbox": (float(mn[0]), float(mn[1]), float(mx[0] - mn[0]), float(mx[1] - mn[1])),
            "desv_unidad": float(np.abs(rej - unidad).max()),
            "regularidad": MOD.regularidad(rej),
        }

    # -- evaluación -------------------------------------------------------- #
    def warp(self, i, k, p, registro=None):
        rej = self.mod.rejillas[i][k]
        neutra = self.mod.rejillas[i][self.kf_neutro[i]]
        uv, dentro = invertir(neutra, p)
        if registro is not None:
            registro.append((self.mod.ids_def[i], k, int((~dentro).sum()), len(p)))
        return MOD.bilineal(rej, np.clip(uv, 0.0, 4.999999))

    def rot(self, i, k, p):
        s = int(self.mod.spec[i])
        kk = int(self.mod.rot_kf_ini[s] + k)
        esc = float(self.mod.rot_esc[kk])
        R = MOD.matriz_rot(float(self.mod.rot_ang[kk]) + float(self.mod.rot_base[s]))
        if self.mod.reflect_x.size:
            p = p * np.array([-1.0 if self.mod.reflect_x[kk] else 1.0,
                              -1.0 if self.mod.reflect_y[kk] else 1.0])
        return esc * (p @ R.T) + np.array([self.mod.rot_ox[kk], self.mod.rot_oy[kk]])

    def evaluar(self, m, kf_malla, combos, diagnostico=False):
        p = self.mod.puntos_malla(m, kf_malla)
        registro = []
        for j, i in enumerate(self.mod.cadenas[m]):
            if self.mod.tipos[i] == 0:
                p = self.warp(i, combos[j], p, registro if diagnostico else None)
            else:
                p = self.rot(i, combos[j], p)
        return p * np.array([1.0, -1.0]), registro

    def error(self, m, kf_malla, combos):
        p, _ = self.evaluar(m, kf_malla, combos)
        return float(np.abs(p - self.oraculo[self.mod.ids_malla[m]]).max())

    def combos_regla(self, m):
        out = []
        for i in self.mod.cadenas[m]:
            banda = int(self.d["deformer.keyform_binding_band_indices"][i])
            out.append(int(self.bnd.indice(banda, int(self.mod.n_kf[i]))))
        return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--moc3",
                    default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--pose", default="/home/evergaster/.hermes/cache/scratch/ora/z10.json")
    ap.add_argument("--neutro", default="regla", choices=["regla", "cero", "regular"])
    ap.add_argument("--todos", action="store_true")
    args = ap.parse_args()

    f = FFD(args.moc3, args.oraculo, args.neutro)
    mod = f.mod

    print(f"jaulas: {len(f.kf_neutro)} warps, keyform neutro = «{args.neutro}»\n")
    print(f"{'warp':<10} {'kf':>3}   {'bbox (x0, y0, w, h)':<34} {'desv [0,1]²':>12} {'regular':>9}")
    filas = [(mod.ids_def[i], f.kf_neutro[i], f.forma_de_la_jaula(i)) for i in sorted(f.kf_neutro)]
    for nombre, k, d in filas:
        b = d["bbox"]
        print(f"{nombre:<10} {k:>3}   ({b[0]:+.3f}, {b[1]:+.3f}, {b[2]:.4f}, {b[3]:.4f})"
              f"   {d['desv_unidad']:>10.5f} {d['regularidad']:>9.4f}")
    exactas = [(n, d) for n, k, d in filas if d["desv_unidad"] < 1e-6]
    print(f"\njaulas que SON el cuadrado unidad: {len(exactas)}/{len(filas)}")

    # ---- reposo ----
    errores = []
    for m in range(mod.n_malla):
        try:
            errores.append((mod.ids_malla[m], len(mod.cadenas[m]),
                            f.error(m, 1, f.combos_regla(m))))
        except Exception as ex:  # noqa: BLE001 - diagnóstico, no producción
            print("  fallo", mod.ids_malla[m], ex)
    errores.sort(key=lambda x: x[2])
    buenas = [e for e in errores if e[2] < 1e-3]
    print(f"\nreposo con la combinación de la regla: < 1e-3 en {len(buenas)}/{len(errores)}"
          f"   mediana {np.median([e[2] for e in errores]):.2e}"
          f"   máximo {errores[-1][2]:.4f}")
    print("  10 mejores:")
    for nombre, n, e in errores[:10]:
        print(f"    {nombre:<12} eslabones={n:<2} error={e:.6f}")
    if args.todos:
        print("\n  todas:")
        for nombre, n, e in errores:
            print(f"    {nombre:<12} eslabones={n:<2} error={e:.6f}")

    # ---- delta entre poses ----
    print(f"\ndelta contra {Path(args.pose).name}:")
    for nombre in ("ArtMesh63", "ArtMesh67", "ArtMesh43", "ArtMesh104"):
        if nombre not in mod.ids_malla:
            continue
        m = mod.ids_malla.index(nombre)
        combos = f.combos_regla(m)
        f.pose(args.oraculo)
        p0, _ = f.evaluar(m, 1, combos)
        o0 = f.oraculo[nombre]
        f.pose(args.pose)
        p10, _ = f.evaluar(m, 1, combos)
        o10 = f.oraculo[nombre]
        dm, do = p10 - p0, o10 - o0
        mov = float(np.linalg.norm(do, axis=1).max())
        err = float(np.linalg.norm(dm - do, axis=1).max())
        pct = (err / mov * 100) if mov else 0.0
        print(f"  {nombre:<12} eslabones={len(mod.cadenas[m]):<2} "
              f"movimiento={mov:.5f}  error_delta={err:.5f}  ({pct:.1f} %)")


if __name__ == "__main__":
    main()
