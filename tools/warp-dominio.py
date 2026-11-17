"""De dónde sale el rectángulo de dominio de un warp, medido (no supuesto).

Para cada caso aislable (un warp cuyo «después» son sólo rotaciones), `warp-aislado.py` ya
recupera el `(u,v)` que usó el Core invirtiendo la rejilla destino sobre la salida del
oráculo. Aquí se hace eso para **todos** los casos, se ajusta `uv = A·a + c` y se despeja el
rectángulo implicado (`w = 5/A_xx`, `h = 5/A_yy`, `x0 = -c_x/A_xx`, …). Después se compara
ese rectángulo con los candidatos que se pueden leer del archivo:

  - rejilla del keyform elegido,
  - rejilla del keyform «identidad» (el más regular),
  - unión de las rejillas de todos los keyforms,
  - caja de los hijos (mallas y warps que cuelgan del warp).

Si alguno coincide en los 50 warps, la regla está ahí. Si ninguno coincide, queda medido que
no sale de la geometría del archivo.

Uso: python3 tools/warp-dominio.py [--modelo …] [--oraculo …]
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


WA = modulo("warp_aislado", "warp-aislado.py")
MOD = WA.MOD


def bbox(pts):
    pts = np.asarray(pts).reshape(-1, 2)
    mn, mx = pts.min(0), pts.max(0)
    return np.array([mn[0], mn[1], mx[0] - mn[0], mx[1] - mn[1]])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--modelo",
                    default=str(RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=str(RAIZ / "models/samples/hiyori.oracle.json"))
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

    por_id = {x["id"]: np.asarray(x["pos"], dtype=float).reshape(-1, 2)
              for x in json.load(open(args.oraculo))["drawables"]}

    # --- candidatos por warp, leídos del archivo (en el espacio del archivo) ---
    def candidatos(i):
        rej_kf = mod.rejillas[i][kf_def[i]].reshape(-1, 2)
        rej_id = mod.rejillas[i][mod.kf_identidad(i)].reshape(-1, 2)
        union = np.concatenate([g.reshape(-1, 2) for g in mod.rejillas[i]])
        hijos = []
        for x in range(mod.n_def):
            if int(mod.padre[x]) == i:
                if mod.tipos[x] == 0:
                    hijos.append(np.concatenate([g.reshape(-1, 2) for g in mod.rejillas[x]]))
        for m in range(mod.n_malla):
            if int(mod.padre_malla[m]) == i:
                hijos.append(mod.puntos_malla(m, kf_malla[m]))
        return {"kf": bbox(rej_kf), "identidad": bbox(rej_id), "union": bbox(union),
                "hijos": bbox(np.concatenate(hijos)) if hijos else None}

    # --- casos aislables ---
    casos = []
    for m in range(mod.n_malla):
        cadena = mod.cadenas[m]
        for j, i in enumerate(cadena):
            if mod.tipos[i] != 0:
                continue
            if any(mod.tipos[x] != 1 for x in cadena[j + 1:]):
                continue
            casos.append((m, j, i))
    print(f"casos aislables (warp con sólo rotaciones después): {len(casos)}")

    medido = {}
    for m, j, i in casos:
        cadena = mod.cadenas[m]
        p = mod.puntos_malla(m, kf_malla[m])
        for x in cadena[:j]:
            if mod.tipos[x] == 0:
                p = MOD.bilineal(mod.rejillas[x][kf_def[x]], p * 5.0)
            else:
                p = WA.aplicar_rot(mod, x, kf_def[x], p)
        b = por_id[mod.ids_malla[m]] * np.array([1.0, -1.0])
        for x in reversed(cadena[j + 1:]):
            b = WA.invertir_rot(mod, x, kf_def[x], b)

        rej = mod.rejillas[i][kf_def[i]]
        uv = WA.invertir_rejilla(rej, b)
        A = np.hstack([p, np.ones((len(p), 1))])
        sol, *_ = np.linalg.lstsq(A, uv, rcond=None)
        residuo = float(np.abs(A @ sol - uv).max())
        if residuo > 1e-3:
            continue
        # rectángulo implicado
        axx, ayy = sol[0, 0], sol[1, 1]
        if abs(axx) < 1e-9 or abs(ayy) < 1e-9:
            continue
        rect = np.array([-sol[2, 0] / axx, -sol[2, 1] / ayy, 5.0 / axx, 5.0 / ayy])
        medido.setdefault(mod.ids_def[i], []).append((rect, abs(sol[0, 1]) + abs(sol[1, 0])))

    warps = {mod.ids_def[i]: i for i in range(mod.n_def) if mod.tipos[i] == 0}
    print(f"warps con rectángulo medido: {len(medido)}\n")
    print(f"{'warp':<10} {'n':>2}  {'rect medido (x0, y0, w, h)':<36} "
          f"{'kf':<26} {'¿=?'}")

    coincidencias = {c: 0 for c in ("kf", "identidad", "union", "hijos")}
    for nombre, lista in sorted(medido.items(), key=lambda kv: -len(kv[1])):
        i = warps[nombre]
        rects = np.array([r for r, _ in lista])
        rect = np.median(rects, axis=0)
        cand = candidatos(i)
        mejor, mejor_d = "—", 1e9
        for clave, c in cand.items():
            if c is None:
                continue
            dd = float(np.abs(c - rect).max())
            if dd < mejor_d:
                mejor, mejor_d = clave, dd
            if dd < 5e-2:
                coincidencias[clave] += 1
        print(f"{nombre:<10} {len(lista):>2}  "
              f"({rect[0]:+.3f}, {rect[1]:+.3f}, {rect[2]:.4f}, {rect[3]:.4f})       "
              f"{(cand['kf'][0]):+.3f},{cand['kf'][2]:.4f}        {mejor} Δ={mejor_d:.3f}")

    print(f"\ncoincidencias (Δ < 5·10⁻²) por candidato, sobre {len(medido)} warps: {coincidencias}")
    print("\ncandidatos, en el orden del guion: rejilla del keyform elegido, rejilla más regular, "
          "unión de keyforms, caja de los hijos.")


if __name__ == "__main__":
    main()
