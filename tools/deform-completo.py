"""El motor completo, según dos implementaciones libres: composición + slots con peso.

Fuentes (independientes del SDK):
  - `AyagamiDev/docs · model/moc3_file_format.md` — comportamiento del formato.
  - `mocari` 0.4.1 (Rust, MIT): `moc3/deformers.rs` (composición), `moc3/compose.rs` (sondeo
    del ángulo y escala acumulada), `core/deformers.rs` (bilineal + extrapolación),
    `moc3/keyform_bindings.rs` (selección **con pesos**, no el keyform más cercano).

Lo que añade esto a `tools/deform-compuesta.py`: la selección de keyform no es «el más
cercano». Por cada parámetro de la banda se busca el tramo que contiene el valor, con
`t = (v - k_i) / (k_{i+1} - k_i)`; el keyform activo es `k_i + (t != 0)` y el peso del
keyform se reparte `(1-t)` / `t` por eje, multiplicando entre ejes. Cuando el valor cae justo
en una clave, queda un solo keyform con peso 1 — y cuando cae en la **última** clave, el
activo es el índice de la derecha, que es justo la excepción de la banda 7 de Hiyori.

Uso: python3 tools/deform-completo.py [--modelo …] [--oraculo …] [--param Nombre=valor]
"""
import argparse
import importlib.util
import itertools
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
# selección de keyform con pesos (keyform_bindings.rs de mocari)
# --------------------------------------------------------------------------- #
class Slots:
    def __init__(self, d):
        self.d = d
        self.kbi = np.asarray(d["keyform_binding_index.indices"])
        self.b_ini = np.asarray(d["keyform_binding_band.begin_indices"])
        self.b_cnt = np.asarray(d["keyform_binding_band.counts"])
        self.k_ini = np.asarray(d["keyform_binding.keys_begin_indices"])
        self.k_cnt = np.asarray(d["keyform_binding.keys_counts"])
        self.keys = np.asarray(d["keys.values"], dtype=float)
        self.defectos = np.asarray(d["parameter.default_values"], dtype=float)
        # parámetro dueño de cada binding: el de begin <= b < siguiente begin mayor
        self.p_ini = np.asarray(d["parameter.keyform_binding_begin_indices"])
        n_bind = int(d["conteos"]["parameter_bindings"])
        self.duenio = [None] * n_bind
        for p, ini in enumerate(self.p_ini):
            ini = int(ini)
            if ini < 0 or ini >= n_bind:
                continue
            fin = n_bind
            for siguiente in self.p_ini[p + 1:]:
                s = int(siguiente)
                if s > ini:
                    fin = s
                    break
            for b in range(ini, min(fin, n_bind)):
                if self.duenio[b] is None:
                    self.duenio[b] = p

    def claves_de(self, banda):
        ini = int(self.b_ini[banda]); cnt = int(self.b_cnt[banda])
        out = []
        for b in self.kbi[ini:ini + cnt]:
            b = int(b)
            ki, kc = int(self.k_ini[b]), int(self.k_cnt[b])
            out.append((self.duenio[b], self.keys[ki:ki + kc], b))
        return out

    def slots(self, banda, n_kf, valores):
        """[(índice local, peso)] — hasta 2^n combinaciones, con sus pesos."""
        if n_kf == 0:
            return []
        if banda < 0:
            return [(0, 1.0)]
        ejes = self.claves_de(banda)
        if not ejes:
            return [(0, 1.0)]
        tramos = []
        stride = 1
        for param, claves, _ in ejes:
            v = float(valores[param]) if param is not None and param < len(valores) else 0.0
            if len(claves) == 1:
                izq, t = 0, 0.0
            elif v <= claves[0]:
                izq, t = 0, 0.0
            elif v >= claves[-1]:
                izq, t = len(claves) - 2, 1.0
            else:
                izq = int(np.searchsorted(claves, v, side="right") - 1)
                izq = min(max(izq, 0), len(claves) - 2)
                t = (v - claves[izq]) / (claves[izq + 1] - claves[izq])
            tramos.append((izq, t, stride, len(claves)))
            stride *= len(claves)
        out = []
        for elecciones in itertools.product(*[(0, 1) for _ in tramos]):
            indice, peso = 0, 1.0
            for (izq, t, st, _), der in zip(tramos, elecciones):
                if der:
                    if t == 0.0:
                        peso = 0.0
                        break
                    indice += (izq + 1) * st
                    peso *= t
                else:
                    indice += izq * st
                    peso *= (1.0 - t)
            if peso > 0.0 and indice < n_kf:
                out.append((indice, peso))
        return out


# --------------------------------------------------------------------------- #
# deformaciones (core/deformers.rs de mocari)
# --------------------------------------------------------------------------- #
def rot_punto(p, ang, esc, origen, fx=False, fy=False):
    th = np.radians(ang); c, s = np.cos(th), np.sin(th)
    sx = -1.0 if fx else 1.0; sy = -1.0 if fy else 1.0
    m = np.array([[c * esc * sx, -s * esc * sy], [s * esc * sx, c * esc * sy]])
    return np.atleast_2d(p) @ m.T + origen


def warp_punto(p, grid, cols, rows):
    x, y = float(p[0]), float(p[1]); stride = cols + 1
    if 0.0 <= x < 1.0 and 0.0 <= y < 1.0:
        u, v = x * cols, y * rows
        i, j = int(u), int(v); s, t = u - i, v - j
        c00 = grid[j * stride + i]; c10 = grid[j * stride + i + 1]
        c01 = grid[(j + 1) * stride + i]; c11 = grid[(j + 1) * stride + i + 1]
        return (1 - s) * (1 - t) * c00 + s * (1 - t) * c10 + (1 - s) * t * c01 + s * t * c11
    c00, c10 = grid[0], grid[cols]
    c01, c11 = grid[rows * stride], grid[rows * stride + cols]
    d11_00 = c11 - c00; d10_01 = c10 - c01
    dpdu = 0.5 * (d11_00 - d10_01); dpdv = 0.5 * (d10_01 + d11_00)
    centro = 0.25 * (c00 + c10 + c01 + c11) - 0.5 * d11_00
    return centro + dpdv * x + dpdu * y


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--modelo",
                    default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--param", action="append", default=[],
                    help="Nombre=valor para mover un parámetro")
    args = ap.parse_args()

    d = MOD.leer_moc3(args.modelo)
    mod = MOD.Modelo(d, json.load(open(args.oraculo)))
    sl = Slots(d)
    ids_param = d["parameter.ids"]
    valores = np.array(sl.defectos, dtype=float)
    for par in args.param:
        nombre, valor = par.split("=")
        valores[ids_param.index(nombre)] = float(valor)

    # slots por malla y por deformador
    slots_malla, slots_def = {}, {}
    for m in range(mod.n_malla):
        banda = int(d["art_mesh.keyform_binding_band_indices"][m])
        slots_malla[m] = sl.slots(banda, int(mod.kf_malla_cnt[m]), valores)
    for i in range(mod.n_def):
        propia = (d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                  else d["rotation_deformer.keyform_binding_band_indices"])
        slots_def[i] = sl.slots(int(propia[int(mod.spec[i])]), int(mod.n_kf[i]), valores)

    def campos_slot(i, k):
        s = int(mod.spec[i]); kk = int(mod.rot_kf_ini[s] + k)
        return (float(mod.rot_ang[kk]), np.array([mod.rot_ox[kk], mod.rot_oy[kk]]),
                float(mod.rot_esc[kk]), kk)

    def rot_interpolada(i):
        ang = 0.0; origen = np.zeros(2); esc = 0.0; kk0 = None
        for k, w in slots_def[i]:
            a, o, e, kk = campos_slot(i, k)
            ang += a * w; origen = origen + o * w; esc += e * w
            if kk0 is None:
                kk0 = kk
        return ang, origen, esc, kk0

    def rejilla_interpolada(i):
        n = int(mod.w_filas[int(mod.spec[i])]) + 1
        c = int(mod.w_cols[int(mod.spec[i])]) + 1
        g = np.zeros((n * c, 2))
        for k, w in slots_def[i]:
            g += mod.rejillas[i][k].reshape(-1, 2) * w
        return g, c - 1, n - 1

    profundidad = {}

    def prof(i):
        if i in profundidad:
            return profundidad[i]
        p = int(mod.padre[i]); profundidad[i] = 0 if p < 0 else prof(p) + 1
        return profundidad[i]

    comp = [None] * mod.n_def

    def aplicar(c, pts):
        if c is None:
            return np.atleast_2d(pts)
        if c[0] == "warp":
            _, grid, cols, rows, _ = c
            return np.array([warp_punto(p, grid, cols, rows) for p in np.atleast_2d(pts)])
        _, ang, esc, origen, fx, fy = c
        return rot_punto(np.atleast_2d(pts), ang, esc, origen, fx, fy)

    def esc_acum(c):
        if c is None:
            return 1.0
        return c[4] if c[0] == "warp" else c[2]

    def sondeo(c, origen, tras):
        if c is None:
            return 0.0
        paso = -0.1 if c[0] == "warp" else -10.0
        fwd = aplicar(c, (tras + np.array([0.0, paso]))[None, :])[0]
        v = fwd - origen
        if np.linalg.norm(v) < 1e-12:
            return 0.0
        return float(np.arctan2(v[1], v[0]) - np.arctan2(paso, 0.0))

    for idx in sorted(range(mod.n_def), key=prof):
        padre = int(mod.padre[idx])
        if not slots_def[idx]:
            comp[idx] = None
            continue
        if mod.tipos[idx] == 0:
            grid, cols, rows = rejilla_interpolada(idx)
            ea = 1.0
            if padre >= 0:
                grid = aplicar(comp[padre], grid)
                ea = esc_acum(comp[padre])
            comp[idx] = ("warp", grid, cols, rows, ea)
        else:
            ang, tras, esc, kk = rot_interpolada(idx)
            ang += float(mod.rot_base[int(mod.spec[idx])])
            if padre >= 0:
                origen = aplicar(comp[padre], tras[None, :])[0]
                des = sondeo(comp[padre], origen, tras)
                ea = esc_acum(comp[padre])
            else:
                origen, des, ea = tras, 0.0, 1.0
            fx = bool(mod.reflect_x[kk]) if mod.reflect_x.size else False
            fy = bool(mod.reflect_y[kk]) if mod.reflect_y.size else False
            comp[idx] = ("rot", ang + np.degrees(des), esc * ea, origen, fx, fy)

    errs = []
    for m in range(mod.n_malla):
        if not slots_malla[m]:
            continue
        p = np.zeros((int(mod.n_vert[m]), 2))
        for k, w in slots_malla[m]:
            p += mod.puntos_malla(m, k) * w
        padre = int(mod.padre_malla[m])
        if padre >= 0:
            p = aplicar(comp[padre], p)
        p = p * np.array([1.0, -1.0])
        o = mod.oraculo[mod.ids_malla[m]]
        if len(o) == len(p):
            errs.append((mod.ids_malla[m], len(mod.cadenas[m]), float(np.abs(p - o).max())))
    errs.sort(key=lambda x: x[2])
    exactas = sum(1 for e in errs if e[2] < 1e-3)
    print(f"completo (composición + slots con peso): < 1e-3 en {exactas}/{len(errs)}   "
          f"< 2 px: {sum(1 for e in errs if e[2] < 6.7e-4)}   "
          f"mediana {np.median([e[2] for e in errs]):.2e}   máximo {errs[-1][2]:.6f}")
    if exactas < len(errs):
        print("  peores:")
        for nombre, n, e in errs[-10:]:
            print(f"   {nombre:<12} eslabones={n:<2} error={e:.6f}")


if __name__ == "__main__":
    main()
