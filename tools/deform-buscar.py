#!/usr/bin/env python3
"""Motor de deformación del `.moc3` — prototipo de búsqueda de fórmulas.

Implementa la cadena malla → deformador → … → raíz para la **pose por defecto** y
la contrasta contra el oráculo del Core (`*.oracle.json`).  El objetivo es medir,
no adivinar: cada variante de fórmula se puntúa con el error máximo por vértice y
se queda la que da cero (o el mínimo posible).

Uso
---
    python3 tools/deform-buscar.py                      # Hiyori + variantes
    python3 tools/deform-buscar.py --modelo X.moc3 --oraculo Y.json
    python3 tools/deform-buscar.py --tabla              # solo la tabla de error
    python3 tools/deform-buscar.py --volcar /tmp/x.json # usa el volcado JSON

Qué está medido (y por qué el código hace lo que hace)
-----------------------------------------------------
1. `keyform_position.xys` es **un pool compartido** de puntos.  El bloque de cada
   keyform empieza en `..._keyform.keyform_position_begin_indices[...]` (en floats)
   y ocupa `ceil(n/8)*8` puntos con `n = art_mesh.position_index_counts` (mallas) o
   `n = 36` (warp 5×5 = 6×6 puntos de control).  `art_mesh.vertex_counts` **no** es
   el número de vértices: es el conteo de índices de triángulo.
2. Mallas y rejillas viven en el espacio **local de su deformador padre**, no en el
   espacio del modelo.  HitArea (sin padre) es la única que coincide con el oráculo
   directamente: sirve de ancla y demuestra que el oráculo entrega `y` invertida
   (`modelo_y = -oráculo_y`).
3. Rotación (verificado a 4e-5 en cadenas sólo-rotación):

       p' = escala · R(ángulo + base_ángulo) · p + (origen_x, origen_y)

   con ángulos en **grados**, R en sentido antihorario y `origen` como traslación
   (no como pivote).  `escala` y `origen` son **por keyform**.
4. Warp: interpolación **bilineal por celda** de la rejilla 6×6 del keyform
   seleccionado, con `(u,v) = (p.x, p.y)` en el cuadrado unidad.  Verificado a 2e-5
   en mallas cuya cadena es warps puros o rotaciones+warps (Hiyori ArtMesh93…
   ArtMesh106, Wanko D_BODY_*).
5. Selección de keyform en la pose por defecto: el índice de la clave cuyo valor
   es `parameter.default_values` de su parámetro, leído por la cadena
   parámetro → `keyform_binding_index.indices` (banda) → banda → claves.

Lo que NO está resuelto (medido, no supuesto)
---------------------------------------------
Las cadenas en las que un warp recibe su entrada **de una rotación** fallan con un
factor no uniforme (Jacobiano 1.7×/1.3× en ArtMesh43): la `(u,v)` del warp no es
`p` en esos casos.  Se probaron y descartaron: rect→rect con el rect del keyform
seleccionado, rect→rect con el rect del keyform identidad, caja envolvente de los
hijos, trasposición de ejes, `v = 1-y`, `u = 1-x`, y Catmull-Rom/bicúbica en vez
de bilineal.  El script informa cuántas mallas cumplen y cuáles no, con su cadena.
"""

import argparse
import json
import math
import os
import struct

import numpy as np

# --------------------------------------------------------------------------- #
# Formato: orden exacto de las secciones del archivo (crates/moc3/src/body.rs)
# --------------------------------------------------------------------------- #
LAYOUT = [
    ("part.runtime_space", "runtime", "parts"),
    ("part.ids", "id", "parts"),
    ("part.keyform_binding_band_indices", "i32", "parts"),
    ("part.keyform_begin_indices", "i32", "parts"),
    ("part.keyform_counts", "i32", "parts"),
    ("part.visibles", "u8", "parts"),
    ("part.enables", "u8", "parts"),
    ("part.parent_part_indices", "i32", "parts"),
    ("deformer.runtime_space", "runtime", "deformers"),
    ("deformer.ids", "id", "deformers"),
    ("deformer.keyform_binding_band_indices", "i32", "deformers"),
    ("deformer.visibles", "u8", "deformers"),
    ("deformer.enables", "u8", "deformers"),
    ("deformer.parent_part_indices", "i32", "deformers"),
    ("deformer.parent_deformer_indices", "i32", "deformers"),
    ("deformer.types", "i32", "deformers"),
    ("deformer.specific_indices", "i32", "deformers"),
    ("warp_deformer.keyform_binding_band_indices", "i32", "warp_deformers"),
    ("warp_deformer.keyform_begin_indices", "i32", "warp_deformers"),
    ("warp_deformer.keyform_counts", "i32", "warp_deformers"),
    ("warp_deformer.vertex_counts", "i32", "warp_deformers"),
    ("warp_deformer.rows", "i32", "warp_deformers"),
    ("warp_deformer.cols", "i32", "warp_deformers"),
    ("rotation_deformer.keyform_binding_band_indices", "i32", "rotation_deformers"),
    ("rotation_deformer.keyform_begin_indices", "i32", "rotation_deformers"),
    ("rotation_deformer.keyform_counts", "i32", "rotation_deformers"),
    ("rotation_deformer.base_angles", "f32", "rotation_deformers"),
    ("art_mesh.runtime_space_0", "runtime", "art_meshes"),
    ("art_mesh.runtime_space_1", "runtime", "art_meshes"),
    ("art_mesh.runtime_space_2", "runtime", "art_meshes"),
    ("art_mesh.runtime_space_3", "runtime", "art_meshes"),
    ("art_mesh.ids", "id", "art_meshes"),
    ("art_mesh.keyform_binding_band_indices", "i32", "art_meshes"),
    ("art_mesh.keyform_begin_indices", "i32", "art_meshes"),
    ("art_mesh.keyform_counts", "i32", "art_meshes"),
    ("art_mesh.visibles", "u8", "art_meshes"),
    ("art_mesh.enables", "u8", "art_meshes"),
    ("art_mesh.parent_part_indices", "i32", "art_meshes"),
    ("art_mesh.parent_deformer_indices", "i32", "art_meshes"),
    ("art_mesh.texture_indices", "i32", "art_meshes"),
    ("art_mesh.drawable_flags", "u8", "art_meshes"),
    ("art_mesh.position_index_counts", "i32", "art_meshes"),
    ("art_mesh.uv_begin_indices", "i32", "art_meshes"),
    ("art_mesh.position_index_begin_indices", "i32", "art_meshes"),
    ("art_mesh.vertex_counts", "i32", "art_meshes"),
    ("art_mesh.mask_begin_indices", "i32", "art_meshes"),
    ("art_mesh.mask_counts", "i32", "art_meshes"),
    ("parameter.runtime_space", "runtime", "parameters"),
    ("parameter.ids", "id", "parameters"),
    ("parameter.max_values", "f32", "parameters"),
    ("parameter.min_values", "f32", "parameters"),
    ("parameter.default_values", "f32", "parameters"),
    ("parameter.repeats", "u8", "parameters"),
    ("parameter.decimal_places", "i32", "parameters"),
    ("parameter.keyform_binding_begin_indices", "i32", "parameters"),
    ("parameter.keyform_binding_counts", "i32", "parameters"),
    ("part_keyform.draw_orders", "f32", "part_keyforms"),
    ("warp_deformer_keyform.opacities", "f32", "warp_deformer_keyforms"),
    ("warp_deformer_keyform.keyform_position_begin_indices", "i32", "warp_deformer_keyforms"),
    ("rotation_deformer_keyform.opacities", "f32", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.angles", "f32", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.origin_xs", "f32", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.origin_ys", "f32", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.scales", "f32", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.reflect_xs", "u8", "rotation_deformer_keyforms"),
    ("rotation_deformer_keyform.reflect_ys", "u8", "rotation_deformer_keyforms"),
    ("art_mesh_keyform.opacities", "f32", "art_mesh_keyforms"),
    ("art_mesh_keyform.draw_orders", "f32", "art_mesh_keyforms"),
    ("art_mesh_keyform.keyform_position_begin_indices", "i32", "art_mesh_keyforms"),
    ("keyform_position.xys", "f32", "keyform_positions"),
    ("keyform_binding_index.indices", "i32", "parameter_binding_indices"),
    ("keyform_binding_band.begin_indices", "i32", "keyform_bindings"),
    ("keyform_binding_band.counts", "i32", "keyform_bindings"),
    ("keyform_binding.keys_begin_indices", "i32", "parameter_bindings"),
    ("keyform_binding.keys_counts", "i32", "parameter_bindings"),
    ("keys.values", "f32", "keys"),
    ("uv.xys", "f32", "uvs"),
    ("position_index.indices", "i16", "position_indices"),
    ("drawable_mask.art_mesh_indices", "i32", "drawable_masks"),
    ("draw_order_group.object_begin_indices", "i32", "draw_order_groups"),
    ("draw_order_group.object_counts", "i32", "draw_order_groups"),
    ("draw_order_group.object_total_counts", "i32", "draw_order_groups"),
    ("draw_order_group.min_draw_orders", "i32", "draw_order_groups"),
    ("draw_order_group.max_draw_orders", "i32", "draw_order_groups"),
    ("draw_order_group_object.types", "i32", "draw_order_group_objects"),
    ("draw_order_group_object.indices", "i32", "draw_order_group_objects"),
    ("draw_order_group_object.group_indices", "i32", "draw_order_group_objects"),
    ("glue.runtime_space", "runtime", "glue"),
    ("glue.ids", "id", "glue"),
    ("glue.keyform_binding_band_indices", "i32", "glue"),
    ("glue.keyform_begin_indices", "i32", "glue"),
    ("glue.keyform_counts", "i32", "glue"),
    ("glue.art_mesh_index_as", "i32", "glue"),
    ("glue.art_mesh_index_bs", "i32", "glue"),
    ("glue.info_begin_indices", "i32", "glue"),
    ("glue.info_counts", "i32", "glue"),
    ("glue_info.weights", "f32", "glue_info"),
    ("glue_info.position_indices", "i16", "glue_info"),
    ("glue_keyform.intensities", "f32", "glue_keyforms"),
]

COUNT_NAMES = [
    "parts", "deformers", "warp_deformers", "rotation_deformers", "art_meshes",
    "parameters", "part_keyforms", "warp_deformer_keyforms",
    "rotation_deformer_keyforms", "art_mesh_keyforms", "keyform_positions",
    "parameter_binding_indices", "keyform_bindings", "parameter_bindings", "keys",
    "uvs", "position_indices", "drawable_masks", "draw_order_groups",
    "draw_order_group_objects", "glue", "glue_info", "glue_keyforms",
]

SIZES = {"i32": 4, "f32": 4, "i16": 2, "u8": 1, "id": 64, "runtime": 0}


def leer_moc3(ruta):
    """Lee las secciones del `.moc3` a **precisión completa** (f32 nativo).

    El volcado a JSON redondea a 6 decimales; con `scales ≈ 1/ppu = 0.000336` ese
    redondeo mete un error relativo de 6e-5 y enmascara el cero.  Leer los bytes
    evita ese suelo.
    """
    datos = open(ruta, "rb").read()
    if datos[:4] != b"MOC3":
        raise SystemExit("no es un .moc3")
    sot = struct.unpack_from("<I", datos, 0x40)[0]
    conteos = {}
    for i, nombre in enumerate(COUNT_NAMES):
        conteos[nombre] = struct.unpack_from("<I", datos, sot + 4 * i)[0]

    def datos_seccion(i):
        off = struct.unpack_from("<I", datos, 0x40 + 4 * (2 + i))[0]
        return off

    out = {"conteos": conteos}
    for i, (nombre, tipo, cuenta) in enumerate(LAYOUT):
        n = conteos[cuenta]
        off = datos_seccion(i)
        if tipo == "runtime" or n == 0 or off == 0:
            continue
        if tipo == "i32":
            out[nombre] = np.array(struct.unpack_from("<%di" % n, datos, off), dtype=np.int32)
        elif tipo == "f32":
            out[nombre] = np.array(struct.unpack_from("<%df" % n, datos, off), dtype=np.float64)
        elif tipo == "i16":
            out[nombre] = np.array(struct.unpack_from("<%dh" % n, datos, off), dtype=np.int32)
        elif tipo == "u8":
            out[nombre] = np.frombuffer(datos, dtype=np.uint8, count=n, offset=off).astype(np.int32)
        elif tipo == "id":
            ids = []
            for k in range(n):
                slot = datos[off + 64 * k: off + 64 * k + 64]
                ids.append(slot.split(b"\0", 1)[0].decode("utf-8", "replace"))
            out[nombre] = ids
    return out


# --------------------------------------------------------------------------- #
# Fórmulas
# --------------------------------------------------------------------------- #
VAR_ROT = {
    "escala·R·p + origen": lambda sc, R, O, p: (sc * (R @ p.T)).T + O,
    "origen + escala·R·(p−origen)": lambda sc, R, O, p: O + (sc * (R @ (p - O).T)).T,
    "R·(p−origen) + origen": lambda sc, R, O, p: (R @ (p - O).T).T + O,
    "escala·R·p + escala·origen": lambda sc, R, O, p: (sc * (R @ p.T)).T + sc * O,
}

VAR_WARP_UV = ("unidad", "rect-keyform", "rect-identidad", "caja-hijos")


def matriz_rot(th_grados):
    th = math.radians(th_grados)
    return np.array([[math.cos(th), -math.sin(th)], [math.sin(th), math.cos(th)]])


def bilineal(rejilla, uv):
    """Interpola la rejilla 6×6 en coordenadas de celda `uv ∈ [0,5]`."""
    cu = np.clip(uv[:, 0], 0.0, 5.0 - 1e-9)
    cv = np.clip(uv[:, 1], 0.0, 5.0 - 1e-9)
    i = np.floor(cu).astype(int)
    j = np.floor(cv).astype(int)
    fu = (cu - i)[:, None]
    fv = (cv - j)[:, None]
    return ((1 - fu) * (1 - fv) * rejilla[j, i] + fu * (1 - fv) * rejilla[j, i + 1]
            + (1 - fu) * fv * rejilla[j + 1, i] + fu * fv * rejilla[j + 1, i + 1])


def regularidad(rejilla):
    """Desviación de la rejilla respecto de una rejilla regular (0 = regular)."""
    dx = np.diff(rejilla[:, :, 0], axis=1)
    dy = np.diff(rejilla[:, :, 1], axis=0)
    return float(np.ptp(dx) + np.ptp(dy))


class Modelo:
    """Cadena de deformadores + evaluación de la pose por defecto."""

    def __init__(self, d, oraculo):
        self.d = d
        c = d["conteos"]
        self.tipos = np.asarray(d["deformer.types"])
        self.padre = np.asarray(d["deformer.parent_deformer_indices"])
        self.spec = np.asarray(d["deformer.specific_indices"])
        self.ids_def = d["deformer.ids"]
        self.ids_malla = d["art_mesh.ids"]
        self.padre_malla = np.asarray(d["art_mesh.parent_deformer_indices"])
        self.n_vert = np.asarray(d["art_mesh.position_index_counts"])
        self.kf_malla_ini = np.asarray(d["art_mesh.keyform_begin_indices"])
        self.kf_malla_cnt = np.asarray(d["art_mesh.keyform_counts"])
        self.kf_malla_pos = np.asarray(d["art_mesh_keyform.keyform_position_begin_indices"])
        self.pool = np.asarray(d["keyform_position.xys"], dtype=np.float64)
        self.n_def = c["deformers"]
        self.n_malla = c["art_meshes"]

        self.rot_kf_ini = np.asarray(d["rotation_deformer.keyform_begin_indices"])
        self.rot_kf_cnt = np.asarray(d["rotation_deformer.keyform_counts"])
        self.rot_ang = np.asarray(d["rotation_deformer_keyform.angles"])
        self.rot_ox = np.asarray(d["rotation_deformer_keyform.origin_xs"])
        self.rot_oy = np.asarray(d["rotation_deformer_keyform.origin_ys"])
        self.rot_esc = np.asarray(d["rotation_deformer_keyform.scales"])
        self.rot_base = np.asarray(d["rotation_deformer.base_angles"])
        self.reflect_x = np.asarray(d.get("rotation_deformer_keyform.reflect_xs", []))
        self.reflect_y = np.asarray(d.get("rotation_deformer_keyform.reflect_ys", []))

        self.w_kf_cnt = np.asarray(d["warp_deformer.keyform_counts"])
        self.w_kf_ini = np.asarray(d["warp_deformer.keyform_begin_indices"])
        self.w_pos = np.asarray(d["warp_deformer_keyform.keyform_position_begin_indices"])
        self.w_filas = np.asarray(d["warp_deformer.rows"])
        self.w_cols = np.asarray(d["warp_deformer.cols"])

        self.oraculo = {x["id"]: np.asarray(x["pos"], dtype=np.float64).reshape(-1, 2)
                        for x in oraculo["drawables"]}

        # rejillas y número de keyforms por elemento
        self.n_kf = np.ones(self.n_def, dtype=int)
        self.rejillas = {}
        for i in range(self.n_def):
            s = int(self.spec[i])
            if self.tipos[i] == 0:
                self.n_kf[i] = int(self.w_kf_cnt[s])
                self.rejillas[i] = [self._rejilla(s, k) for k in range(self.n_kf[i])]
            else:
                self.n_kf[i] = int(self.rot_kf_cnt[s])

        self.cadenas = []
        for m in range(self.n_malla):
            cadena, p = [], int(self.padre_malla[m])
            while p != -1:
                cadena.append(p)
                p = int(self.padre[p])
            self.cadenas.append(cadena)

    # -- acceso a datos ---------------------------------------------------- #
    def _rejilla(self, s, k):
        b = int(self.w_pos[self.w_kf_ini[s] + k]) // 2
        filas, cols = int(self.w_filas[s]) + 1, int(self.w_cols[s]) + 1
        return self.pool[2 * b: 2 * (b + filas * cols)].reshape(filas, cols, 2)

    def puntos_malla(self, m, kf):
        n = int(self.n_vert[m])
        b = int(self.kf_malla_pos[self.kf_malla_ini[m] + kf]) // 2
        return self.pool[2 * b: 2 * (b + n)].reshape(-1, 2).copy()

    # -- evaluación -------------------------------------------------------- #
    def evaluar(self, m, kf_malla, kf_def, var_rot="escala·R·p + origen", var_uv="unidad"):
        p = self.puntos_malla(m, kf_malla)
        for i in self.cadenas[m]:
            if self.tipos[i] == 0:
                rej = self.rejillas[i][kf_def[i]]
                if var_uv == "unidad":
                    uv = p * 5.0
                else:
                    ref = rej if var_uv == "rect-keyform" else self.rejillas[i][self.kf_identidad(i)]
                    mn = ref.reshape(-1, 2).min(0)
                    mx = ref.reshape(-1, 2).max(0)
                    uv = (p - mn) / np.maximum(mx - mn, 1e-12) * 5.0
                p = bilineal(rej, uv)
            else:
                s = int(self.spec[i])
                k = int(self.rot_kf_ini[s] + kf_def[i])
                sc = float(self.rot_esc[k])
                R = matriz_rot(float(self.rot_ang[k]) + float(self.rot_base[s]))
                if self.reflect_x.size:
                    p = p * np.array([-1.0 if self.reflect_x[k] else 1.0,
                                      -1.0 if self.reflect_y[k] else 1.0])
                O = np.array([self.rot_ox[k], self.rot_oy[k]])
                p = VAR_ROT[var_rot](sc, R, O, p)
        return p * np.array([1.0, -1.0])   # el oráculo entrega y invertida

    def kf_identidad(self, i):
        """Keyform más cercano a la rejilla regular (heurística para el rect)."""
        return int(np.argmin([regularidad(g) for g in self.rejillas[i]]))

    def error(self, m, kf_malla, kf_def, **kw):
        return float(np.abs(self.evaluar(m, kf_malla, kf_def, **kw) - self.oraculo[self.ids_malla[m]]).max())


# --------------------------------------------------------------------------- #
# Selección de keyform por parámetro
# --------------------------------------------------------------------------- #
class Bindings:
    """parámetro → banda → claves, y elemento → banda.

    Medido (238/238 elementos de Hiyori cuadran): `keyform_binding_index.indices[s]`
    **no** es una banda, es el **índice de la lista de claves** del slot `s`.  La
    banda `b` ocupa los slots `[b_ini[b], b_ini[b]+b_cnt[b])`; el número de keyforms
    del elemento es el producto de los tamaños de esas listas, y el índice de
    keyform de la pose por defecto es el radix de la posición del `default` en cada
    lista, en el orden de la banda:

        kf = Σ_i  pos_i · Π_{j<i} len(claves_j)
    """

    def __init__(self, d):
        self.d = d
        self.params = d["parameter.ids"]
        self.defaults = np.asarray(d["parameter.default_values"])
        self.kbi = np.asarray(d["keyform_binding_index.indices"])
        self.p_ini = np.asarray(d["parameter.keyform_binding_begin_indices"])
        self.p_cnt = np.asarray(d["parameter.keyform_binding_counts"])
        self.b_ini = np.asarray(d["keyform_binding_band.begin_indices"])
        self.b_cnt = np.asarray(d["keyform_binding_band.counts"])
        self.k_ini = np.asarray(d["keyform_binding.keys_begin_indices"])
        self.k_cnt = np.asarray(d["keyform_binding.keys_counts"])
        self.keys = np.asarray(d["keys.values"])
        self.slot_a_param = {}
        for p in range(len(self.params)):
            for s in range(int(self.p_ini[p]), int(self.p_ini[p] + self.p_cnt[p])):
                self.slot_a_param[s] = p

    def claves_de(self, b):
        """Listas de claves de la banda, en el orden del radix."""
        out = []
        for s in range(int(self.b_ini[b]), int(self.b_ini[b] + self.b_cnt[b])):
            k = int(self.kbi[s])
            out.append((self.slot_a_param.get(s),
                        self.keys[self.k_ini[k]: self.k_ini[k] + self.k_cnt[k]]))
        return out

    def indice(self, banda, n_kf, regla="clave"):
        """Índice de keyform de la pose por defecto (radix sobre la banda)."""
        if regla == "medio":
            return (n_kf - 1) // 2
        lista = self.claves_de(banda)
        if not lista:
            return 0
        if regla == "prop" and len(lista) == 1 and len(lista[0][1]) > 1:
            p, ks = lista[0]
            v = self.defaults[p] if p is not None else 0.0
            t = (v - ks[0]) / (ks[-1] - ks[0])
            return int(np.clip(round(t * (n_kf - 1)), 0, n_kf - 1))
        pos, mul = 0, 1
        for p, ks in lista:
            v = self.defaults[p] if p is not None else 0.0
            i = int(np.argmin(np.abs(ks - v)))
            pos += i * mul
            mul *= len(ks)
        return min(pos, int(n_kf) - 1)


# --------------------------------------------------------------------------- #
# Búsqueda de la selección (medida contra el oráculo, sin adivinar)
# --------------------------------------------------------------------------- #
def buscar_seleccion(mod, regla="clave", umbral=1e-3, verbose=True):
    """Resuelve el keyform de cada elemento en la pose por defecto.

    Recorre las mallas de menor a mayor espacio de búsqueda.  Los deformadores que
    ya tienen un valor **confirmado** (≥2 mallas coinciden y el ajuste es < umbral)
    se fijan; el resto se busca por fuerza bruta sobre su subcadena.
    """
    import itertools

    bnd = Bindings(mod.d)
    kfs = {}
    for i in range(mod.n_def):
        if mod.tipos[i] == 0:
            banda = int(mod.d["warp_deformer.keyform_binding_band_indices"][int(mod.spec[i])])
        else:
            banda = int(mod.d["rotation_deformer.keyform_binding_band_indices"][int(mod.spec[i])])
        kfs[i] = bnd.indice(banda, int(mod.n_kf[i]), regla)
    kf_malla = {}
    for m in range(mod.n_malla):
        banda = int(mod.d["art_mesh.keyform_binding_band_indices"][m])
        kf_malla[m] = bnd.indice(banda, int(mod.kf_malla_cnt[m]), regla)

    votos = {i: {} for i in range(mod.n_def)}

    def confirmado(i):
        v = votos[i]
        if not v:
            return None
        mejor = max(v.items(), key=lambda kv: len(kv[1]))
        return mejor[0] if len(mejor[1]) >= 2 else None

    orden = sorted(range(mod.n_malla),
                   key=lambda m: np.prod([mod.n_kf[i] for i in mod.cadenas[m]]) or 1)
    for m in orden:
        cadena = mod.cadenas[m]
        fijos = {i: confirmado(i) for i in cadena if confirmado(i) is not None}
        libres = [i for i in cadena if i not in fijos]
        espacio = int(np.prod([mod.n_kf[i] for i in libres])) if libres else 1
        mejor = (1e9, kf_malla[m], {})
        if espacio <= 4000:
            for combo in itertools.product(*[range(int(mod.n_kf[i])) for i in libres]):
                kd = dict(kfs); kd.update(fijos); kd.update(dict(zip(libres, combo)))
                e, k = min((mod.error(m, kk, kd), kk) for kk in range(int(mod.kf_malla_cnt[m])))
                if e < mejor[0]:
                    mejor = (e, k, dict(zip(libres, combo)))
        else:  # descenso por coordenadas con varios arranques
            for arranque in [tuple(0 for _ in libres)] + [
                    tuple((j * 3 + a) % int(mod.n_kf[i]) for j, i in enumerate(libres))
                    for a in range(1, 12)]:
                kd = dict(kfs); kd.update(fijos); kd.update(dict(zip(libres, arranque)))
                e, k = min((mod.error(m, kk, kd), kk) for kk in range(int(mod.kf_malla_cnt[m])))
                for _ in range(10):
                    movido = False
                    for i in libres:
                        actual = kd[i]
                        for kk2 in range(int(mod.n_kf[i])):
                            kd[i] = kk2
                            e2, k2 = min((mod.error(m, q, kd), q) for q in range(int(mod.kf_malla_cnt[m])))
                            if e2 < e:
                                e, k, actual = e2, k2, kk2
                        kd[i] = actual
                        movido |= actual != kd[i]
                    if not movido:
                        break
                if e < mejor[0]:
                    mejor = (e, k, {i: kd[i] for i in libres})
        kf_malla[m] = mejor[1]
        if mejor[0] < umbral:
            for i, v in mejor[2].items():
                votos[i].setdefault(v, []).append(m)
    for i in range(mod.n_def):
        v = votos[i]
        if v:
            kfs[i] = max(v.items(), key=lambda kv: len(kv[1]))[0]
    return kf_malla, kfs


# --------------------------------------------------------------------------- #
# Programa
# --------------------------------------------------------------------------- #
def main():
    ap = argparse.ArgumentParser(description="Motor de deformación .moc3: búsqueda de fórmulas")
    raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap.add_argument("--modelo", default=os.path.join(raiz, "models/samples/Samples/Resources/Hiyori/Hiyori.moc3"))
    ap.add_argument("--oraculo", default=os.path.join(raiz, "models/samples/hiyori.oracle.json"))
    ap.add_argument("--volcar", help="volcado JSON ya generado (pierde precisión)")
    ap.add_argument("--tabla", action="store_true", help="imprime la tabla de error por malla")
    ap.add_argument("--umbral", type=float, default=1e-3)
    ap.add_argument("--json", help="escribe la tabla de error en JSON")
    ap.add_argument("--sin-busqueda", action="store_true", help="sólo la regla de parámetros")
    args = ap.parse_args()

    d = json.load(open(args.volcar)) if args.volcar else leer_moc3(args.modelo)
    oraculo = json.load(open(args.oraculo))
    mod = Modelo(d, oraculo)
    print("modelo: %s  (%d mallas, %d deformadores, oráculo %s)"
          % (os.path.basename(args.modelo), mod.n_malla, mod.n_def, os.path.basename(args.oraculo)))

    # 1) variantes de fórmula con la selección por parámetro
    bnd = Bindings(d)
    kf_malla = {}
    kfs = {}
    for m in range(mod.n_malla):
        banda = int(d["art_mesh.keyform_binding_band_indices"][m])
        kf_malla[m] = bnd.indice(banda, int(mod.kf_malla_cnt[m]))
    for i in range(mod.n_def):
        banda = int((d["warp_deformer.keyform_binding_band_indices"] if mod.tipos[i] == 0
                     else d["rotation_deformer.keyform_binding_band_indices"])[int(mod.spec[i])])
        kfs[i] = bnd.indice(banda, int(mod.n_kf[i]))

    print("\n== variantes de fórmula (selección por parámetro) ==")
    print("%-34s %-14s %7s %10s %10s" % ("rotación", "warp (u,v)", "<1e-3", "máx", "mediana"))
    mejor = None
    for var_rot in VAR_ROT:
        for var_uv in VAR_WARP_UV:
            errs = np.array([mod.error(m, kf_malla[m], kfs, var_rot=var_rot, var_uv=var_uv)
                             for m in range(mod.n_malla)])
            n = int((errs < args.umbral).sum())
            print("%-34s %-14s %7d %10.2e %10.2e"
                  % (var_rot, var_uv, n, errs.max(), np.median(errs)))
            if mejor is None or n > mejor[0]:
                mejor = (n, var_rot, var_uv)
    n, var_rot, var_uv = mejor
    print("ganadora: rotación «%s», warp «%s» (%d/%d)" % (var_rot, var_uv, n, mod.n_malla))

    if not args.sin_busqueda:
        print("\n== búsqueda de la selección de keyform (medida contra el oráculo) ==")
        kf_malla, kfs = buscar_seleccion(mod, umbral=args.umbral)

    errs = np.array([mod.error(m, kf_malla[m], kfs, var_rot=var_rot, var_uv=var_uv)
                     for m in range(mod.n_malla)])
    dentro = errs < args.umbral
    print("\n== resultado ==")
    print("mallas con error < %.0e: %d/%d   (< 2 px = 6.7e-4: %d)"
          % (args.umbral, dentro.sum(), mod.n_malla, (errs < 6.7e-4).sum()))
    print("error máximo %.3e   mediana %.3e" % (errs.max(), np.median(errs)))

    # clasificación por forma de la cadena (dónde se rompe la fórmula)
    def clase(cad):
        visto_warp = False
        for i in cad:
            if mod.tipos[i] == 0:
                visto_warp = True
            elif visto_warp:
                return "warp→rotación→warp"
        return "warps y luego rotaciones" if visto_warp else "sólo rotaciones"

    print("\n== por forma de cadena ==")
    for cl in ("sólo rotaciones", "warps y luego rotaciones", "warp→rotación→warp"):
        idx = [m for m in range(mod.n_malla) if clase(mod.cadenas[m]) == cl]
        if not idx:
            continue
        e = errs[idx]
        print("%-26s %3d mallas   <umbral %2d   mediana %.2e   mín %.2e"
              % (cl, len(idx), int((e < args.umbral).sum()), np.median(e), e.min()))

    if args.json:
        tabla = [{"malla": mod.ids_malla[m], "vertices": int(mod.n_vert[m]),
                  "kf_malla": int(kf_malla[m]),
                  "kf_deformadores": {mod.ids_def[i]: int(kfs[i]) for i in mod.cadenas[m]},
                  "error": float(errs[m]),
                  "cadena": [mod.ids_def[i] for i in mod.cadenas[m]]}
                 for m in np.argsort(-errs)]
        json.dump({"modelo": args.modelo, "variante_rot": var_rot, "variante_warp": var_uv,
                   "umbral": args.umbral, "dentro": int(dentro.sum()),
                   "total": mod.n_malla, "error_max": float(errs.max()),
                   "tabla": tabla}, open(args.json, "w"), indent=1)
        print("\ntabla escrita en %s" % args.json)

    if args.tabla:
        print("\n%-16s %4s %3s %10s  %s" % ("malla", "n", "kf", "error", "cadena"))
        for m in np.argsort(-errs):
            cadena = "→".join("%s%d" % ("W" if mod.tipos[i] == 0 else "R", kfs[i]) for i in mod.cadenas[m])
            print("%-16s %4d %3d %10.2e  %s"
                  % (mod.ids_malla[m], int(mod.n_vert[m]), kf_malla[m], errs[m], cadena))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
