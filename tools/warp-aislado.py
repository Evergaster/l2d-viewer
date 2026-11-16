"""Aísla el warp: con la rotación ya validada (docs/13), se invierte la parte de la cadena
que va DESPUÉS del warp y se aplica la que va ANTES. Lo que queda son pares
(entrada, salida) reales del warp, medidos contra el Core, sin conjeturas.

Uso: python3 tools/warp-aislado.py [modelo.moc3] [oraculo.json]
"""
import importlib.util
import json
import sys
from pathlib import Path

import numpy as np

RAIZ = Path(__file__).resolve().parent.parent


def cargar_modulo():
    ruta = RAIZ / "tools" / "deform-buscar.py"
    spec = importlib.util.spec_from_file_location("deform_buscar", ruta)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MOD = cargar_modulo()


def aplicar_rot(mod, i, k, p):
    """adelante: la rotación validada de docs/13."""
    s = int(mod.spec[i])
    kk = int(mod.rot_kf_ini[s] + k)
    esc = float(mod.rot_esc[kk])
    R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
    if mod.reflect_x.size:
        p = p * np.array([-1.0 if mod.reflect_x[kk] else 1.0,
                          -1.0 if mod.reflect_y[kk] else 1.0])
    O = np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
    return esc * (p @ R.T) + O


def invertir_rot(mod, i, k, p):
    """atrás: la misma rotación, deshecha."""
    s = int(mod.spec[i])
    kk = int(mod.rot_kf_ini[s] + k)
    esc = float(mod.rot_esc[kk])
    R = MOD.matriz_rot(float(mod.rot_ang[kk]) + float(mod.rot_base[s]))
    O = np.array([mod.rot_ox[kk], mod.rot_oy[kk]])
    q = (p - O) / esc
    q = q @ R          # R^T por la derecha = R^{-1} por la izquierda
    if mod.reflect_x.size:
        q = q * np.array([-1.0 if mod.reflect_x[kk] else 1.0,
                          -1.0 if mod.reflect_y[kk] else 1.0])
    return q


def invertir_rejilla(rej, pts, iteraciones=40):
    """Devuelve (u,v) en coordenadas de celda [0,5] tal que bilineal(rej, uv) = pts.

    Es el paso que falta en la hipótesis «unidad»: el dominio real del warp puede ser su
    rejilla de reposo, no el cuadrado [0,1]². Newton con Jacobiano numérico.
    """
    mn = rej.reshape(-1, 2).min(0)
    mx = rej.reshape(-1, 2).max(0)
    uv = np.clip((pts - mn) / np.maximum(mx - mn, 1e-12) * 5.0, 0.0, 5.0)
    h = 1e-4
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


def main():
    ruta = sys.argv[1] if len(sys.argv) > 1 else str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3")
    oraculo = sys.argv[2] if len(sys.argv) > 2 else str(
        RAIZ / "models/samples/hiyori.oracle.json")

    d = MOD.leer_moc3(ruta)
    mod = MOD.Modelo(d, json.load(open(oraculo)))

    # keyform elegido por defecto (regla de docs/13: índice de la lista de claves)
    bnd = MOD.Bindings(d)
    kf_def = {}
    for i in range(mod.n_def):
        propio = (d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                  else d["rotation_deformer.keyform_binding_band_indices"])
        kf_def[i] = bnd.indice(int(propio[int(mod.spec[i])]), int(mod.n_kf[i]))
    kf_malla = {}
    for m in range(mod.n_malla):
        kf_malla[m] = bnd.indice(int(d["art_mesh.keyform_binding_band_indices"][m]),
                                 int(mod.kf_malla_cnt[m]))

    casos = []
    for m in range(mod.n_malla):
        cadena = mod.cadenas[m]
        for j, i in enumerate(cadena):
            if mod.tipos[i] != 0:      # sólo warps
                continue
            antes, despues = cadena[:j], cadena[j + 1:]
            # antes: cualquier cosa (se aplica en orden); después: sólo rotaciones (para invertir)
            if any(mod.tipos[x] != 1 for x in despues):
                continue
            casos.append((m, j, i, antes, despues))

    print(f"cadenas con un warp cuyo 'después' es sólo rotaciones: {len(casos)}")
    if not casos:
        print("no hay casos: prueba con otro modelo")
        return

    # oráculo en el espacio del archivo (y invertida)
    por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
              for x in json.load(open(oraculo))["drawables"]}

    reposo = {i: mod.rejillas[i][mod.kf_identidad(i)] for i in range(mod.n_def) if mod.tipos[i] == 0}

    filas = []
    for m, j, i, antes, despues in casos:
        p = mod.puntos_malla(m, kf_malla[m])
        for x in antes:
            if mod.tipos[x] == 0:
                rej = mod.rejillas[x][kf_def[x]]
                p = MOD.bilineal(rej, p * 5.0)
            else:
                p = aplicar_rot(mod, x, kf_def[x], p)
        a = p                                    # entrada real del warp
        b = por_id[mod.ids_malla[m]] * np.array([1.0, -1.0])   # oráculo al espacio del archivo
        for x in reversed(despues):
            b = invertir_rot(mod, x, kf_def[x], b)
        # ¿qué warp explica a -> b?
        rej = mod.rejillas[i][kf_def[i]]
        e_unidad = float(np.abs(MOD.bilineal(rej, a * 5.0) - b).max())
        mn, mx = a.min(0), a.max(0)
        e_bbox = float(np.abs(MOD.bilineal(rej, (a - mn) / np.maximum(mx - mn, 1e-12) * 5.0) - b).max())
        e_reposo = float(np.abs(MOD.bilineal(rej, invertir_rejilla(reposo[i], a)) - b).max())
        # ¿qué (u,v) usó el Core? Se recupera invirtiendo la rejilla destino sobre b.
        # Después se mira si (u,v) es afín en la entrada: uv = A·a + c.
        uv = invertir_rejilla(rej, b)
        A = np.hstack([a, np.ones((len(a), 1))])
        sol, *_ = np.linalg.lstsq(A, uv, rcond=None)
        res = float(np.abs(A @ sol - uv).max())
        filas.append((e_unidad, e_bbox, e_reposo, mod.ids_malla[m], mod.ids_def[i],
                      len(antes) + 1 + len(despues), mn, mx, sol, res))

    filas.sort(key=lambda f: f[9])
    print(f"\n{'residuo uv~a':>12} {'u=5a':>9} {'malla':>12} {'warp':>10} {'n':>3}   "
          f"A (d uv / d a) y c")
    for f in filas[:12]:
        sol = f[8]
        print(f"{f[9]:12.6f} {f[0]:9.4f} {f[3]:>12} {f[4]:>10} {f[5]:3}   "
              f"[[{sol[0,0]:+.3f},{sol[1,0]:+.3f}],[{sol[0,1]:+.3f},{sol[1,1]:+.3f}]] "
              f"c=({sol[2,0]:+.4f},{sol[2,1]:+.4f})")

    e1s = [f[0] for f in filas]
    e2s = [f[1] for f in filas]
    e3s = [f[2] for f in filas]
    rs = [f[9] for f in filas]
    print(f"\nresiduo de uv = A·a + c: mediana {np.median(rs):.6f}, mejor {min(rs):.6f} "
          f"({sum(1 for r in rs if r < 1e-3)}/{len(rs)} casos con residuo < 1e-3)")
    print(f"u = 5·a (dominio cuadrado):     {sum(1 for e in e1s if e < 1e-3)}/{len(filas)} "
          f"(mediana {np.median(e1s):.4f})")
    print(f"caja de la entrada:             {sum(1 for e in e2s if e < 1e-3)}/{len(filas)} "
          f"(mediana {np.median(e2s):.4f})")
    print(f"rejilla de reposo invertida:    {sum(1 for e in e3s if e < 1e-3)}/{len(filas)} "
          f"(mediana {np.median(e3s):.4f})")


if __name__ == "__main__":
    main()
