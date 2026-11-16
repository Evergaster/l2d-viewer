"""¿El Core mezcla también los keyforms de la MALLA? Es la pieza que falta en docs/13 §5.2-5.3.

Se evaluaba la cadena partiendo del bloque de un solo keyform de la malla. Si el Core interpola
también las posiciones de la malla entre dos keyforms vecinos, el mapa deja de ser afín (que es
exactamente el residuo medido en ArtMesh102/106) y además mejora todas las demás.

Variantes:
  M0  malla sin mezcla + rotación sin mezcla          (lo que había: 77/134)
  M1  malla mezclada + rotación sin mezcla
  M2  malla mezclada + rotación mezclada (salidas)
  M3  malla sin mezcla + rotación mezclada

Uso: python3 tools/pose-mezcla.py [modelo.moc3] [oraculo.json]
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


def peso(claves, v, modo="tramo"):
    claves = np.asarray(claves, dtype=float)
    if len(claves) < 2:
        return 0, 0, 0.0
    if v <= claves[0]:
        return 0, 0, 0.0
    if v >= claves[-1]:
        return len(claves) - 1, len(claves) - 1, 0.0
    j = int(np.searchsorted(claves, v) - 1)
    return j, j + 1, float((v - claves[j]) / (claves[j + 1] - claves[j]))


def main():
    ruta = sys.argv[1] if len(sys.argv) > 1 else str(
        RAIZ / "models/samples/Samples/Resources/Hiyori/Hiyori.moc3")
    oraculo = sys.argv[2] if len(sys.argv) > 2 else str(
        RAIZ / "models/samples/hiyori.oracle.json")

    d = MOD.leer_moc3(ruta)
    mod = MOD.Modelo(d, json.load(open(oraculo)))
    por_id = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
              for x in json.load(open(oraculo))["drawables"]}
    bnd = MOD.Bindings(d)

    dominios = {}
    if Path("/tmp/dominios-warp.json").exists():
        nombres = json.load(open("/tmp/dominios-warp.json"))
        dominios = {i: tuple(nombres[mod.ids_def[i]]) for i in range(mod.n_def)
                    if mod.tipos[i] == 0 and mod.ids_def[i] in nombres}

    # --- malla: bloque de posiciones por keyform ---
    def bloque(m, k):
        n = int(mod.n_vert[m])
        b = int(mod.kf_malla_pos[mod.kf_malla_ini[m] + k]) // 2
        return mod.pool[2 * b: 2 * (b + n)].reshape(-1, 2).copy()

    def vertices_malla(m, mezclar):
        banda = int(d["art_mesh.keyform_binding_band_indices"][m])
        lista = bnd.claves_de(banda)
        if not mezclar or not lista:
            return bloque(m, bnd.indice(banda, int(mod.kf_malla_cnt[m])))
        p, claves = lista[0]
        v = float(bnd.defaults[p]) if p is not None else 0.0
        j, j1, w = peso(claves, v)
        if j == j1:
            return bloque(m, j)
        return (1 - w) * bloque(m, j) + w * bloque(m, j1)

    # --- rotación ---
    def campos(i, k):
        s = int(mod.spec[i])
        kk = int(mod.rot_kf_ini[s] + k)
        return (float(mod.rot_ang[kk]), float(mod.rot_ox[kk]),
                float(mod.rot_oy[kk]), float(mod.rot_esc[kk]))

    def una(i, k, p):
        ang, ox, oy, esc = campos(i, k)
        R = MOD.matriz_rot(ang + float(mod.rot_base[int(mod.spec[i])]))
        return esc * (p @ R.T) + np.array([ox, oy])

    def rotacion(i, p, mezclar):
        banda = int(d["rotation_deformer.keyform_binding_band_indices"][int(mod.spec[i])])
        lista = bnd.claves_de(banda)
        n = int(mod.n_kf[i])
        if not mezclar or not lista:
            return una(i, bnd.indice(banda, n), p)
        pr, claves = lista[0]
        v = float(bnd.defaults[pr]) if pr is not None else 0.0
        j, j1, w = peso(claves, v)
        if j == j1:
            return una(i, j, p)
        return (1 - w) * una(i, j, p) + w * una(i, j1, p)

    def evaluar(m, mezcla_malla, mezcla_rot):
        p = vertices_malla(m, mezcla_malla)
        for i in mod.cadenas[m]:
            if mod.tipos[i] == 1:
                p = rotacion(i, p, mezcla_rot)
            else:
                k = bnd.indice(int(d["warp_deformer.keyform_binding_band_indices"][int(mod.spec[i])]),
                               int(mod.n_kf[i]))
                rej = mod.rejillas[i][k]
                if i in dominios:
                    x0, y0, w, h = dominios[i]
                    uv = (p - np.array([x0, y0])) / np.array([w, h]) * 5.0
                else:
                    uv = p * 5.0
                p = MOD.bilineal(rej, uv)
        return p * np.array([1.0, -1.0])

    print(f"{'variante':>34} {'<1e-3':>7} {'<6.7e-4':>9} {'mediana':>10} {'máx':>10}")
    for etiqueta, mm, mr in [("malla sin mezcla + rot sin mezcla", False, False),
                             ("malla MEZCLADA + rot sin mezcla", True, False),
                             ("malla MEZCLADA + rot MEZCLADA", True, True),
                             ("malla sin mezcla + rot MEZCLADA", False, True)]:
        errs = np.array([float(np.abs(evaluar(m, mm, mr) - por_id[mod.ids_malla[m]]).max())
                         for m in range(mod.n_malla)])
        print(f"{etiqueta:>34} {int((errs < 1e-3).sum()):7} {int((errs < 6.7e-4).sum()):9} "
              f"{np.median(errs):10.5f} {errs.max():10.5f}")

    for nombre in ["ArtMesh102", "ArtMesh106", "ArtMesh88"]:
        if nombre not in mod.ids_malla:
            continue
        m = mod.ids_malla.index(nombre)
        print(f"\n{nombre}:")
        for etiqueta, mm, mr in [("sin mezcla", False, False), ("malla mezclada", True, False),
                                 ("malla+rot mezcladas", True, True)]:
            err = float(np.abs(evaluar(m, mm, mr) - por_id[nombre]).max())
            print(f"   {etiqueta:>20}: {err:.6f}")


if __name__ == "__main__":
    main()
