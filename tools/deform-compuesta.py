"""La composición **deformador a deformador**, tal como la documenta una implementación libre.

Fuentes (independientes, no el SDK):
  - `AyagamiDev/docs · moc3_file_format.md` § «Deformer chaining» (el caso especial).
  - `mocari` (crate Rust, docs.rs `mocari/moc3/deformers.rs` + `core/deformers.rs`), que trae
    el algoritmo completo: se compone cada deformador con el suyo padre y **la malla sólo se
    transforma por su padre directo**, no por la cadena entera.

Las dos piezas que nuestro motor no tenía:

1. `compose`: se procesa de raíz a hoja. La rejilla de un warp se deforma con el padre
   compuesto; una rotación hereda la **escala acumulada** del padre y su ángulo se corrige
   con un **sondeo** (`+ (0, -0.1)` deformado por el padre: la rotación del vector resultante
   es el desfase).
2. `warp_deformer_transform_target`: dentro de `[0,1)²` es bilineal; **fuera** hay
   extrapolación con una base afín de las esquinas (no un recorte al borde).

Uso: python3 tools/deform-compuesta.py [--modelo …] [--oraculo …]
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
# piezas geométricas (core/deformers.rs de mocari)
# --------------------------------------------------------------------------- #
def rot_punto(p, ang_grados, escala, origen, reflect_x=False, reflect_y=False):
    th = np.radians(ang_grados)
    c, s = np.cos(th), np.sin(th)
    sx = -1.0 if reflect_x else 1.0
    sy = -1.0 if reflect_y else 1.0
    m = np.array([[c * escala * sx, -s * escala * sy],
                  [s * escala * sx, c * escala * sy]])
    return p @ m.T + origen


def warp_punto(p, grid, cols, rows):
    """Bilineal por celda con `u = x·cols`; extrapolación afín fuera de la rejilla."""
    x, y = float(p[0]), float(p[1])
    stride = cols + 1
    if 0.0 <= x < 1.0 and 0.0 <= y < 1.0:
        u, v = x * cols, y * rows
        i, j = int(u), int(v)
        s, t = u - i, v - j
        c00 = grid[j * stride + i]
        c10 = grid[j * stride + i + 1]
        c01 = grid[(j + 1) * stride + i]
        c11 = grid[(j + 1) * stride + i + 1]
        return ((1 - s) * (1 - t) * c00 + s * (1 - t) * c10
                + (1 - s) * t * c01 + s * t * c11)

    # fuera: base afín derivada de las esquinas (spec §extrapolación)
    c00, c10 = grid[0], grid[cols]
    c01, c11 = grid[rows * stride], grid[rows * stride + cols]
    d11_00 = c11 - c00
    d10_01 = c10 - c01
    dpdu = 0.5 * (d11_00 - d10_01)
    dpdv = 0.5 * (d10_01 + d11_00)
    centro = 0.25 * (c00 + c10 + c01 + c11) - 0.5 * d11_00
    # aproximación: extensión afín desde el centro (la spec afina el perímetro;
    # aquí basta para medir si la composición era el problema)
    return centro + dpdv * x + dpdu * y


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--modelo",
                    default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
    ap.add_argument("--secuencial", action="store_true", help="referencia: cadena punto a punto")
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

    profundidad = {}
    def prof(i):
        if i in profundidad:
            return profundidad[i]
        p = int(mod.padre[i]); profundidad[i] = 0 if p < 0 else prof(p) + 1
        return profundidad[i]

    def campos(i, k):
        s = int(mod.spec[i]); kk = int(mod.rot_kf_ini[s] + k)
        return (float(mod.rot_ang[kk]), np.array([mod.rot_ox[kk], mod.rot_oy[kk]]),
                float(mod.rot_esc[kk]), kk)

    def aplicar(c, puntos):
        if c is None:
            return puntos
        if c[0] == "warp":
            _, grid, cols, rows, _ = c
            return np.array([warp_punto(p, grid, cols, rows) for p in puntos])
        _, ang, esc, origen, fx, fy = c
        return rot_punto(np.atleast_2d(puntos), ang, esc, origen, fx, fy)

    def escala_acum(c):
        if c is None:
            return 1.0
        return c[4] if c[0] == "warp" else c[2]

    def sondeo(c, origen, traslacion):
        """desfase de ángulo por el padre, con el sondeo de -0.1 en Y (mocari)."""
        if c is None:
            return 0.0
        paso = -0.1 if c[0] == "warp" else -10.0
        fwd = aplicar(c, (traslacion + np.array([0.0, paso]))[None, :])[0]
        v = fwd - origen
        if np.linalg.norm(v) < 1e-12:
            return 0.0
        return float(np.arctan2(v[1], v[0]) - np.arctan2(paso, 0.0))

    # ---- composición de raíz a hoja ----
    comp = [None] * mod.n_def        # ('warp', grid, cols, rows, escala) | ('rot', ...)
    for idx in sorted(range(mod.n_def), key=prof):
        padre = int(mod.padre[idx])
        if mod.tipos[idx] == 0:
            w = mod.warps if hasattr(mod, "warps") else None
            del w
            filas = int(mod.w_filas[int(mod.spec[idx])]) + 1
            cols = int(mod.w_cols[int(mod.spec[idx])]) + 1
            grid = mod.rejillas[idx][kf_def[idx]].reshape(-1, 2).copy()
            esc_acum = 1.0
            if padre >= 0:
                grid = aplicar(comp[padre], grid)
                esc_acum = escala_acum(comp[padre])
            comp[idx] = ("warp", grid, cols - 1, filas - 1, esc_acum)
        else:
            ang, traslacion, esc, kk = campos(idx, kf_def[idx])
            ang += float(mod.rot_base[int(mod.spec[idx])])
            if padre >= 0:
                origen = aplicar(comp[padre], traslacion[None, :])[0]
                desfase = sondeo(comp[padre], origen, traslacion)
                esc_acum = escala_acum(comp[padre])
            else:
                origen, desfase, esc_acum = traslacion, 0.0, 1.0
            fx = bool(mod.reflect_x[kk]) if mod.reflect_x.size else False
            fy = bool(mod.reflect_y[kk]) if mod.reflect_y.size else False
            comp[idx] = ("rot", ang + np.degrees(desfase), esc * esc_acum, origen, fx, fy)

    # ---- evaluación ----
    errs = []
    for m in range(mod.n_malla):
        p = mod.puntos_malla(m, kf_malla[m])
        padre = int(mod.padre_malla[m])
        if args.secuencial:
            for i in mod.cadenas[m]:
                p = aplicar(comp[i], p)
        elif padre >= 0:
            p = aplicar(comp[padre], p)
        p = p * np.array([1.0, -1.0])
        o = mod.oraculo[mod.ids_malla[m]]
        if len(o) == len(p):
            errs.append((mod.ids_malla[m], len(mod.cadenas[m]), float(np.abs(p - o).max())))

    errs.sort(key=lambda x: x[2])
    etiqueta = "secuencial (referencia)" if args.secuencial else "compuesta (mocari)"
    print(f"{etiqueta}: < 1e-3 en {sum(1 for e in errs if e[2] < 1e-3)}/{len(errs)}   "
          f"< 2 px: {sum(1 for e in errs if e[2] < 6.7e-4)}   "
          f"mediana {np.median([e[2] for e in errs]):.2e}   máximo {errs[-1][2]:.4f}")
    for nombre, n, e in errs[:10]:
        print(f"   {nombre:<12} eslabones={n:<2} error={e:.6f}")
    print("   5 peores:")
    for nombre, n, e in errs[-5:]:
        print(f"   {nombre:<12} eslabones={n:<2} error={e:.6f}")


if __name__ == "__main__":
    main()
