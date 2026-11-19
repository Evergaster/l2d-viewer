# 20 — Orden de dibujo: los grupos del archivo, medidos

Cierra el último hueco de formato que quedaba en la tabla del README (`draw_order_group ⬜`). El
visor ya no adivina el orden de pintado: lo saca del archivo y lo desempata como dice la
especificación. La mejora está medida contra el Core.

---

## 1. Lo que dice la especificación

De `AyagamiDev/docs · model/moc3_file_format.md` (no oficial, pero coincidió con el Core en todo lo
demás):

> ArtMeshes are nominally drawn in order sorted by depth value, from high to low depth (back to
> front). However, since the depth may be equal and the sort is stable, a model needs to declare the
> base order for its ArtMeshes. This is the job of draw groups.
>
> To draw the model the draw group tree is traversed, and at each node its children are
> depth-sorted. The sorted tree is then flattened to a list of sorted ArtMeshes to be drawn.

Y el detalle que hace falta para no equivocarse: las profundidades se **redondean a entero**
(suma 0,001 y trunca), así que los empates no son raros.

Traducido: **profundidad primero, árbol del archivo como desempate.** El orden del archivo *solo*
no es el orden de pintado.

---

## 2. Lo que se leyó

`crates/moc3/src/body.rs` ahora expone:

| Campo | Sección del `.moc3` |
|---|---|
| `grupos[i].objetos` | `draw_order_group.object_begin_indices` + `object_counts` |
| `grupos[i].total_mallas` | `draw_order_group.object_total_counts` |
| `grupos[i].min_depth` / `max_depth` | `draw_order_group.min/max_draw_orders` |
| `objetos_grupo[i].tipo` / `indice` / `grupo_hijo` | `draw_order_group_object.types` / `indices` / `group_indices` |

El tipo 0 resultó ser la malla (medido, no supuesto: en Hiyori los 134 items son tipo 0 y el árbol
cubre las 134 mallas exactamente una vez). `grupo_indices` negativo = el item no es un subgrupo.

Dos herramientas dejadas para volver a medir sin escribir código nuevo:

```bash
cargo run --release -p moc3 --example grupos -- <modelo.moc3>          # árbol + comparación de órdenes
cargo run --release -p view --example comparar -- <modelo.moc3> 560    # dos PNG y % de píxeles distintos
```

---

## 3. La medición (y el orden que casi elijo mal)

La referencia es el **Core oficial**: `l2d oracle` pinta la geometría y el `renderOrder` que el Core
entrega en su volcado (`models/samples/hiyori.oracle.json`), con **nuestro** rasterizador. Así la
comparación aísla el orden: misma geometría, mismo pintor, distinto criterio de orden.

| Variante de orden | Diferencia media vs Core | Píxeles con Δ > 60 |
|---|---|---|
| Solo profundidad (lo que hacía el visor) | 38,54 | 13,56 % |
| Árbol del archivo **tal cual** | 57,73 | 19,33 % |
| **Profundidad + árbol como desempate** | **19,41** | **8,61 %** |

Por regiones (la del medio es la que más mejora, y es donde están los solapes de ropa):

| Zona | Solo profundidad | Con árbol |
|---|---|---|
| cabeza | 17,78 | **12,70** |
| torso | 53,81 | **28,05** |
| piernas | 28,33 | **10,01** |

Dos cosas que se aprendieron midiendo:

1. **Aplanar el árbol en el orden del archivo empeora las cosas** (57,7). Es el orden *base*, no el
   final: si no se ordena por profundidad dentro de cada grupo, se pinta al revés en la mitad del
   cuerpo. Ese fue el primer intento y lo descarté con el número, no con una intuición.
2. **La descripción de una imagen no sirve como juez.** Preguntado por la variante ganadora, un
   modelo de visión contestó que tenía "el pelo cortado y el cuello flotando"; por la anterior, que
   estaba perfecta. La métrica contra el Core dice lo contrario en las tres regiones. Cuando hay
   referencia oficial, se compara con ella.

---

## 4. Decisión

`Orden::Arbol` es el **orden por defecto** (`render` y `render_encuadre` lo usan; `Orden::Profundidad`
queda como sonda para volver a medir).

| Regla | Dónde |
|---|---|
| dentro de cada grupo, ordenar por profundidad; desempatar por la posición en el árbol | `render_orden`, rama `Orden::Arbol` |
| mallas que el archivo no lista: van al final del desempate, con orden por índice | `posicion[i] = usize::MAX` |
| archivo sin grupos: se cae al orden por profundidad | `orden_por_grupos` devuelve vacío |

Tres pruebas nuevas (`crates/view/tests/orden.rs`): el árbol cubre cada malla exactamente una vez
(también en Wanko), el orden por defecto es el del árbol y difiere del de profundidad sola, y cambiar
de orden no cambia **qué** se dibuja (la silueta aguanta dentro del 1 %).

---

## 5. Lo que queda de la diferencia

Queda un 8,6 % de píxeles que no coinciden con el Core. Sospechosos, sin medir todavía:

- **Sin antialias**: el rasterizador es software sin MSAA; el Core pinta con MSAA. Todos los bordes
  difieren por construcción.
- **Blend modes por malla** (multiplicativo/aditivo): el visor pinta todo con `Normal`.
- **Profundidad interpolada del keyform activo**: aquí se usa la del keyform elegido por la regla de
  pesos, que ya es la correcta (`docs/16`), pero conviene medirlo en poses movidas, no solo en
  reposo.

Ninguno de los tres es de orden; el orden ya está cerrado.
