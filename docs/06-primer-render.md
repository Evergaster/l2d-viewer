# 06 — Primer render real (F2.1)

**Fecha:** 2026-11-13. **Objetivo:** pasar de "parsea el contenedor" a "dibuja un modelo".

## Lo que ya sale

```
$ l2d render Hiyori.moc3 Hiyori.2048/texture_00.png out/hiyori.png 0.3
render out/hiyori.png  893x1253  122 mallas, 3356 triángulos  (caja 295x1051)
```

El pelo, la ropa y las texturas se colocan en su sitio. **La cara no**: falta la cabeza y hay
piezas desplazadas. Eso no es un fallo del parser — es que todavía se dibuja el *primer keyform*
de cada malla, no el keyform que corresponde a los parámetros por defecto.

## Semántica real de los campos (deducida, no supuesta)

Comparando los valores con la implementación de referencia `py-moc3` y con los datos crudos:

| Campo | Lo que parecía | Lo que es |
|---|---|---|
| `art_mesh.vertex_counts` | nº de vértices | **nº de entradas de índice** (3 por triángulo) |
| `art_mesh.position_index_counts` | nº de índices | **nº de triángulos** solo en algunas mallas → no se usa |
| `art_mesh.uv_begin_indices` | índice de pares UV | **índice en floats** dentro de `uv.xys` |
| `art_mesh.position_index_begin_indices` | offset de índices | ✅ correcto (saltos = `vertex_counts`) |
| número de vértices | — | **`max(indices) + 1`** de la malla |
| `keyform_position.xys` | — | pool plano de floats (x, y), compartido con warp deformers |

Verificación cruzada: para las 34 mallas de Wanko, `begin[i+1] - begin[i] == vertex_counts[i]`
siempre; y `uv_begin[i+1] - uv_begin[i] == 2 * max_index+1` en las mallas contiguas. Eso cierra
la semántica de los tres campos.

## Cómo se dibuja hoy

1. Se recogen todas las mallas visibles, con sus vértices del **keyform 0** (`keyform_begin`).
2. Se calcula la caja de todos los vértices y se encaja en el canvas (auto-fit, 95 %).
   El `pixels-per-unit` real sigue sin identificarse en el bloque de canvas, así que **no** se
   usa una escala "oficial": se encaja la caja. Es una decisión de vista, y está documentada
   como tal.
3. Se ordenan por `draw_order` del keyform y se rasterizan con el rasterizador CPU de `raster`.

## Lo que falta para que la cara aparezca

| # | Tarea | Por qué |
|---|---|---|
| 1 | Leer `keyform_binding_band` + `keys` + `parameter_bindings` | elegir el keyform que corresponde a los valores por defecto de cada parámetro |
| 2 | Aplicar el keyform elegido en vez del primero | es la causa directa de las piezas desplazadas |
| 3 | Máscaras (`drawable_mask`) | Hiyori tiene 8; sin ellas se ven partes que deberían estar ocultas |
| 4 | Blend modes (`drawable_flags`) | normales, aditivos y multiplicativos |
| 5 | Deformadores (warp/rotation) | mover un parámetro y ver la deformación |

Las tareas 1–2 son las que convierten esto en un visor. Las 3–5, en un visor correcto.

## Progreso: el keyform central

Cambiar el keyform 0 por el **central** de cada malla (`keyform_begin + keyform_count/2`) hizo
aparecer cabeza, pelo y torso en Hiyori.

Después se añadió el **orden por parte**: `part.visibles` para descartar mallas cuya parte padre
está oculta, y `part_keyform.draw_orders` como clave primaria de orden (con el draw order de la
malla como desempate). El resultado ya tiene las capas correctas: pelo delante, ropa detrás.

Lo que queda para la cara:

- **Selección de keyform por parámetro** (abajo). Es *el* bloqueador: las 24 partes de Hiyori
  están visibles y ninguna malla se descarta, así que la cara se está dibujando… en otro sitio.
- **Órdenes de dibujo empatados:** 134 mallas con solo 13 valores distintos; los empates los
  resuelve hoy el orden del archivo, no `draw_order_group` / `draw_order_group_object`.

## Máscaras (hechas)

Se leen `drawable_mask.art_mesh_indices` y `art_mesh.mask_begin/count`. Para cada malla con
máscaras se rasterizan primero esas mallas en un canvas aparte (blanco opaco) y luego el alfa de
la malla se multiplica por el alfa de la máscara (`raster::Canvas::triangle_masked`).

Resultado: `render … 125 mallas, 3366 triángulos, 8 con máscara`. Visualmente **no cambió la
cara**, porque el problema no era el recorte: es la posición. Se queda implementado y medido.

## Lo que se sabe de los bindings (a medias, a propósito)

Datos reales de Wanko: 25 parámetros, 25 entradas de binding, **21 bandas**, 126 claves.

- Cada parámetro tiene un rango en `keyform_binding_index.indices` que apunta a una banda.
- Cada banda tiene claves: p. ej. `PARAM_ANGLE_X` → banda 11 → `[-10, 0, 40, 50]`.
- Pero `keyform_count` de una malla **no** coincide con el número de claves de su banda
  (2 de 34 mallas coinciden). Es decir: los keyforms de una malla son el **producto** de varias
  bandas, y hay que deshacer el índice en radix mixto.

Eso es exactamente lo que queda por implementar en la tarea 1: el índice de keyform es una
combinación `(k1, k2, …)` sobre las bandas de la malla, y el keyform neutro es el que tiene
todas las claves en su valor por defecto. Sin ese cálculo, «el central» es una aproximación que
funciona a veces (Hiyori) y falla en otras (Wanko cambió de 34 a 30 mallas dibujadas).

## Honestidad sobre el estado

- El render **es real**: usa los vértices y UVs del archivo, y la textura del modelo.
- El render **no es todavía fiel**: falta la selección de keyform y las máscaras.
- Wanko (34 mallas, sin máscaras) sale mucho más coherente que Hiyori (134 mallas, 8 máscaras).

## Salidas de esta sesión

- `out/wanko.png` — 34 mallas, 399 triángulos.
- `out/hiyori.png` — 122 mallas, 3356 triángulos.

Ambas locales, no versionadas: los modelos son de muestra del SDK y no se redistribuyen.
