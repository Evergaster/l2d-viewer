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

---

## 4. Warp aislado: la entrada se recupera y es afín (16 nov, segunda tanda)

`tools/warp-aislado.py` aísla el warp: aplica la parte de la cadena que va **antes** de él y
**invierte** la que va después (sólo rotaciones, ya validadas). Quedan pares reales
`(entrada a, salida b)` medidos contra el Core: **129 casos** en Hiyori.

Con esos pares se recupera el `(u,v)` que usó el Core invirtiendo la rejilla destino sobre `b`
(Newton con Jacobiano numérico: `bilineal(rej, uv) = b`). Resultado medido:

| Hipótesis de dominio | Casos con error < 1e-3 |
|---|---|
| `u = 5·a`, `v = 5·a` (cuadrado unidad) | **8/129** (mediana 0,034) |
| caja de la entrada (bbox) | 0/129 (mediana 0,72) |
| rejilla de reposo invertida | 0/129 (mediana 0,72) |
| **`uv = A·a + c` (afín, ajustado)** | **109/129 con residuo < 1e-3** |

El `(u,v)` que usa el Core **es afín en la entrada** y el mismo para todas las mallas que cuelgan
del mismo warp. Para `Warp1` (el warp raíz de Hiyori):

```
A = [[8.789, ~0], [~0, 6.266]]      c = (-1.876, -0.4516)
⇒ rectángulo del dominio:  x ∈ [0.2135, 0.7824]  (ancho 0.5689)
                           y ∈ [0.0721, 0.8700]  (alto  0.7980)
```

Comparado con la rejilla de reposo de ese warp (`kf` más regular: ancho 0.7346, alto 1.0305):
el rectángulo del dominio es **0,774 × la caja de la rejilla en los dos ejes**, con una traslación.
Ese factor es el dato que falta para generalizar.

### Lo que queda, con el camino corto

1. Medir `(A, c)` para **todos** los warps (se puede: un warp también se invierte numéricamente con
   la misma inversión de rejilla), no sólo para el warp raíz.
2. Ver a qué dato del archivo corresponde ese rectángulo — candidatos, en orden de probabilidad:
   caja de los hijos en su espacio, rejilla de reposo escalada por un factor fijo, o el rect que se
   deduce de las claves de la banda.
3. Con la regla, la fórmula del warp queda cerrada y se puede implementar en `crates/pose`.

Los tres dominios descartados están medidos, no supuestos: no hace falta volver a probarlos.

---

## 5. Calibración de los 50 dominios (16 nov, tercera tanda)

`tools/deform-calibrar.py` resuelve el rectángulo de dominio de **todos** los warps por descenso
por bloques: parte de «cuadrado unidad» en los 50, y en cada ronda vuelve a medir cada warp
usando las estimaciones actuales de los demás (prefijo hacia adelante, sufijo invertido). No
adivina la regla: mide los rectángulos que hacen que la cadena reproduzca el Core.

| Ronda | Mallas con error < 1e-3 | Mediana | Máximo |
|---|---|---|---|
| inicio (todo cuadrado unidad) | 12/134 | 3,3·10⁻² | 1,2·10⁻¹ |
| 1 | 78/134 | 5,6·10⁻⁴ | 1,2·10⁻¹ |
| 2–6 (estable) | 77/134 | **3,0·10⁻⁴** | 1,2·10⁻¹ |

La mediana ya está en **0,9 px** (ppu = 2976), por debajo del criterio de 2 px de `docs/10` §8.
76 mallas cumplen el criterio estricto. Los 12 dominios impresos muestran la estructura:

- La mayoría de los warps calibran en **cuadrado unidad** (0, 1) — los que ya funcionaban.
- `Warp1` (raíz): `x0=+0.2136 y0=+0.0719 w=0.5690 h=0.7982`.
- `Warp47..Warp50`: `w=1.7576 h=1.2529` — y **1.7576 = 1/0.5689**, el recíproco del ancho de
  `Warp1`. No es casualidad: los rectángulos de la familia están relacionados por las cajas de
  las rejillas, pero el cociente contra mi «keyform más regular» sale disperso (0,001 a 17,7),
  así que la heurística de rejilla de reposo **no** sirve para deducir la regla.

### Los dos fallos que quedan, con nombre

1. **`ArtMesh102` y `ArtMesh106`** (cadenas de sólo rotaciones, error 0,078 y 0,092): es la
   excepción de selección de keyform que ya había detectado la búsqueda — banda 7 de
   `Rotation21/22/23`, donde el Core usa el índice 1 y la regla del producto da 0.
2. **Cadenas largas que pasan por `Warp1`** (`ArtMesh88`, `ArtMesh83`, `ArtMesh91`, `ArtMesh75`,
   `ArtMesh80`, 0,06–0,07): cinco a ocho deformadores encadenados; el error se acumula o el
   dominio de un eslabón intermedio no es afín. Es el siguiente objetivo.

### 5.1 La excepción de keyform no es sólo la selección (medido)

Probadas **todas** las combinaciones de keyform para las dos mallas de sólo rotaciones:

| Malla | Combinación de la regla | Mejor combinación medida | Error de la mejor |
|---|---|---|---|
| `ArtMesh102` (Rot22→Rot21→Rot24) | (0, 0, 1) → 0,078 | **(1, 1, 2)** | **0,0065** (~19 px) |
| `ArtMesh106` (Rot23→Rot22→Rot21→Rot24) | (0, 0, 0, 1) → 0,092 | **(1, 1, 1, 0)** | **0,0063** (~19 px) |

Dos conclusiones:

1. La selección de keyform sí está mal ahí: la regla elige 0 donde el Core usa 1 (banda 7, claves
   en `keys[22..23]`). Cambiarla baja el error **12×**.
2. Pero **no lo explica todo**: aun con la mejor combinación quedan 0,0065 unidades. Queda un error
   sistemático pequeño en la rotación (candidatos: `reflect` en algún eslabón, o el origen como
   pivote en algunos deformadores). Ese es el siguiente hilo, y es medible igual que los demás.
