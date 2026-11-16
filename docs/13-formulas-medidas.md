# 13 — Motor de deformación: fórmulas medidas (2026-11-16)

Continúa `docs/12`. Aquí está lo que se **midió** contra el oráculo del Core con
`tools/deform-buscar.py`, no lo que se supuso. Estado: la rotación y la selección de
keyform quedan resueltas y verificadas; el warp queda resuelto **sólo** cuando su
entrada viene de una malla u otro warp, y ahí está el bloqueo que queda abierto.

---

## 1. Fórmula ganadora

```
paseo de la cadena:  malla → deformador padre → … → raíz

rotación (deformer.type = 1, objetivos en grados):
    p' = escala · R(ángulo + base_ángulo) · p + (origen_x, origen_y)

warp (deformer.type = 0, rejilla 6×6 = 5×5 celdas):
    p' = bilineal( rejilla_kf , u = 5·p.x , v = 5·p.y )        # u,v en celdas

al final (convenio del oráculo):
    y_modelo = − y_oráculo
```

- `escala`, `ángulo`, `origen_x/y`, `reflect_x/y` son **por keyform** de rotación;
  `base_ángulo` es por deformador. Los ángulos están en grados y `R` es antihoraria.
- `origen` entra como **traslación**, no como pivote: la variante
  `origen + escala·R·(p−origen)` también da error pequeño pero peor (10/134 frente a
  12/134 y mediana 4,8e-2 frente a 3,3e-2); `R·(p−origen)+origen` y
  `escala·R·p + escala·origen` fallan con 2e2–9e-1.
- El warp es **bilineal por celda**, no Bézier ni Catmull-Rom: con la rejilla 6×6 de
  B-spline/Bézier y con Catmull-Rom los errores suben (0,44–0,59 en las mallas que
  sí cuadran con bilineal).
- `p` para el warp es la coordenada **tal cual** en el cuadrado unidad del espacio de
  su hijo (no relativa al rect de la rejilla, ni a la caja de los hijos, ni con ejes
  cambiados: las cuatro alternativas dan 6e-1 de mediana).

## 2. Selección de keyform en la pose por defecto (resuelta, 238/238)

`keyform_binding_index.indices[s]` **no** es un índice de banda: es el **índice de la
lista de claves** del slot `s`. La banda `b` ocupa los slots
`[begin_indices[b], begin_indices[b] + counts[b])`, y cada slot apunta a una lista de
claves en `keyform_binding.keys_begin_indices/keys_counts` + `keys.values`.

Verificación: para los **238** elementos (134 mallas + 50 warps + 54 rotaciones) el
producto de los tamaños de las listas de su banda es **exactamente** su
`keyform_counts` declarado. 238/238.

El índice de la pose por defecto es el radix de la posición del `default` del
parámetro en cada lista, en el orden de la banda:

```
kf = Σ_i  pos_i · Π_{j<i} len(claves_j)      pos_i = índice de default en claves_i
```

Contrastado con la selección hallada por fuerza bruta contra el oráculo: coincide en
todas las mallas y deformadores comprobados salvo tres rotaciones de Hiyori
(`Rotation21/22/23`, banda 7, claves `[0, 1]`, 2 keyforms) que el Core resuelve con
índice 1 y la regla da 0 — 11 de 12 casos. Ese es el único punto abierto de la
selección.

## 3. Precisión: hay que leer los bytes, no el volcado JSON

`volcar` redondea a 6 decimales. Con `scales ≈ 1/ppu` (0,000336 en Hiyori, 0,000833 en
Wanko) eso mete un error relativo de 6e-5 y deja un **suelo de ~1,5e-4** en el error
por vértice: se ve en Wanko, donde las 10 mallas de rotación pura dan 4,8e-6 con
f32 nativo y 1,5e-4 con el JSON. `deform-buscar.py` lee el `.moc3` directamente
(`--volcar` existe sólo para comparar).

## 4. Qué cuadra y qué no (Hiyori, 134 mallas)

| forma de la cadena | mallas | error < 1e-3 | mediana |
|---|---|---|---|
| sólo rotaciones | 5 | **5** (todas, 3e-6–5e-6) | 5,0e-6 |
| warps y luego rotaciones | 58 | 8 | 4,6e-2 |
| warp → rotación → warp | 71 | 2 | 3,0e-2 |

Global: **15/134 con error < 1e-3**, máximo 7,6e-2, mediana 3,2e-2. De esas 15, catorce
están por debajo de 1e-5 (precisión de f32): son las mallas de la cabeza
`ArtMesh93…ArtMesh106` más `HitArea`. Wanko: 12/34, con 10/11 de las cadenas de
rotación pura en 5e-6.

Las 15 exactas cubren las tres formas que sí se pueden reproducir: cadena sólo de
warps, sólo de rotaciones, y warp→rotaciones. El patrón de fallo es nítido: **toda
cadena en la que una rotación alimenta a un warp**.

## 5. El bloqueo que queda (con la evidencia medida)

Para `ArtMesh43` (cadena R1 → R2 → Warp → Warp0 → Warp1) el Jacobiano del oráculo es
`1/2976 · I` (la malla conserva su tamaño de bloque: 26,25 px frente a 26,23), mientras
la fórmula predice `1/2976 · diag(0,57 ; 0,77)` por los rects de los tres warps. El
ajuste afín de la salida de la fórmula contra el oráculo da exactamente eso:
`diag(1,74 ; 1,30)` más una traslación, con error residual 3e-3. Es decir: en esa
cadena los warps se comportan como **identidad** para la coordenada que entra, no como
`[0,1]² → rect`. Con las mismas tres rejillas, en una cadena sólo de warps
(`ArtMesh93`) la fórmula `[0,1]² → rect` da 5e-6, así que el `(u,v)` depende de dónde
venga la entrada y no se ha identificado la regla.

Descartado midiendo, no opinando: rect del keyform seleccionado, rect del keyform
identidad, caja envolvente de los hijos, `u=1−x`, `v=1−y`, ejes intercambiados,
Bézier (Bernstein 5×5), Catmull-Rom, `u,v` divididos por 272/1200/2976, y forzar
`rect→rect` sólo en los warps alimentados por rotación. Ninguna baja el error de las
71 mallas del tercer grupo.

## 6. Cómo se reproduce

```
cargo run -q -p moc3 --example volcar -- models/samples/Samples/Resources/Hiyori/Hiyori.moc3 > /tmp/hiyori.json
python3 tools/deform-buscar.py                     # tabla de variantes + tabla de error
python3 tools/deform-buscar.py --sin-busqueda      # sólo la regla de keyform (0,6 s)
python3 tools/deform-buscar.py --json /tmp/t.json  # tabla por malla en JSON
python3 tools/deform-buscar.py --modelo …/Wanko.moc3 --oraculo models/samples/wanko.oracle.json
```
