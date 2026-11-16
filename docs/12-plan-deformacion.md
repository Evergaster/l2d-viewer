# 12 — Plan del motor de deformación (2026-11-16)

Respuesta corta a «¿se puede escribir el algoritmo de deformación?»: **sí**, y los datos están
todos en el archivo. Lo que no se puede es adivinarlo: hay dos cosas medidas hoy que invalidan la
lectura ingenua, y este documento las deja escritas para no repetir el camino.

> **Continuación:** el primer intento de implementar las fórmulas está en `docs/13-formulas-medidas.md`
> (rotación y selección de keyform resueltas; el warp, sólo cuando su entrada no viene de una
> rotación). El grafo y el layout del pool ya son código en `crates/pose`.

---

## 1. Lo que ya está leído y verificado (Hiyori.moc3)

`cargo run -p moc3 --example volcar -- <moc3>` vuelca a JSON las 99 secciones con datos. Sobre
Hiyori:

| Pieza | Números |
|---|---|
| `deformer.types` | 104 deformadores: **50 warp (tipo 0)** y **54 rotation (tipo 1)** |
| Rejilla de los warp | **todos 5×5**, `vertex_counts` = 36 puntos de control |
| `deformer.parent_deformer_indices` | la cadena padre, y `art_mesh.parent_deformer_indices` engancha cada malla a su deformador |
| `deformer.specific_indices` | índice dentro de la tabla de su tipo (warp 0..49, rotation 0..53) |
| Keyforms | 271 de warp + 151 de rotation + 1242 de malla |
| Rotación por keyform | `angles`, `origin_xs/ys`, `scales`, `reflect_xs/ys` + `base_angles` |
| Enlaces a parámetros | 75 bandas, 90 índices de banda, 212 claves, 70 parámetros con su rango de bindings |

Es decir: **el modelo de datos del motor está completo** — cadena de deformadores, rejillas,
keyforms de los tres tipos y el binding parámetro→banda→clave. No falta ninguna pieza en el
formato.

---

## 2. Dos cosas que impiden la lectura ingenua

### 2.1 El pool de posiciones no se recorre con `vertex_counts`

`keyform_position.xys` es **un solo pool compartido** por keyforms de malla y de warp. Los pasos
reales entre bloques no coinciden con los conteos declarados:

| Medición | Esperado | Real |
|---|---|---|
| Bloque de un keyform de warp (5×5 → 36 puntos) | 36 | **40** (`begin` = 0, 80, 160, 240… floats) |
| Bloque de un keyform de malla | nº de vértices de la malla (19–26) | **24–32** puntos |

Conclusión: el tamaño de bloque no sale de `vertex_counts`. Hay un empaquetado (o un campo extra por
keyform) que hay que deducir de la estructura del pool, no del conteo declarado. Esto es, muy
probablemente, la causa del «desacuerdo de índice» que `docs/07` y `docs/10` §6 dejaron abierto:
66 mallas leían bloques en escala de píxeles.

### 2.2 Las posiciones del pool están en espacio local del deformador padre

Medido con `tools/buscar-bloques.py` (busca en el pool el bloque que mejor reproduce la geometría
del oráculo con un ajuste **rígido**: escala uniforme + rotación + traslación):

- Ajuste **afín libre**: 131/134 mallas cuadran a error < 1e-4 → **falso positivo**, el ajuste es
  demasiado permisivo (acepta bloques ajenos girándolos y escalándolos).
- Ajuste **rígido**: **0/134** cuadran. Error mínimo 0,0007 unidades (~2 px) y mediana ~0,003.

Lo segundo es la respuesta correcta y coincide con `docs/07`: **ningún bloque del pool es la
geometría final**. Antes de aplicar la cadena de deformadores, comparar contra el oráculo no puede
dar cero. Por eso la calibración contra el oráculo no sirve como primer paso: el primer paso es
aplicar la cadena.

---

## 3. Orden de trabajo para `crates/pose`

1. **Estructura primero.** Leer deformadores (tipo, padre, rejilla) y montar la cadena
   malla → deformador → … → raíz. Sin posiciones todavía: solo el grafo.
2. **Layout del pool.** Deducir el tamaño real de bloque (40 puntos en warp, 24–32 en malla) y
   escribir el iterador que devuelve el bloque de un keyform. Verificación: los bloques de un mismo
   deformador deben ser todos del mismo tamaño y no solaparse.
3. **Warp.** Interpolación de la rejilla: para un punto dentro de una celda, con sus coordenadas
   `(u,v)` normalizadas, ¿bilineal de las 4 esquinas o bezier de los 4 puntos de control? Se decide
   **midiendo**: con un solo warp en la cadena y su keyform de reposo, las dos fórmulas tienen que
   dar la identidad sobre la rejilla; después, con un keyform deformado, se compara contra el
   oráculo y gana la que dé < 2 px.
4. **Rotation.** Afín con `angle`, `origin`, `scale`, `reflect` y `base_angle`. Es la parte barata:
   es una matriz 2×3, se valida con una malla cuyo padre sea una sola rotación.
5. **Cadena completa.** Componer padre→hijo y validar **todas** las mallas de Hiyori y Wanko contra
   el oráculo con el criterio de `docs/10` §8: error < 2 px por vértice.
6. **Keyform por parámetro** (lo que queda): el radix de bandas. Con la cadena funcionando, el
   oráculo sirve por fin como juez: se mueve un parámetro, se comparan las posiciones finales y se
   verifica qué clave eligió el Core.

---

## 4. Herramientas que quedan en el repo

| Herramienta | Para qué |
|---|---|
| `cargo run -p moc3 --example volcar -- <moc3> > /tmp/modelo.json` | volcar las 99 secciones a JSON |
| `python3 tools/buscar-bloques.py <volcado.json> <oracle.json>` | probar hipótesis sobre el pool contra la verdad del Core |

El oráculo se genera con `models/samples/oraculo.html` servido por HTTP (patrón en `docs/07`).

---

## 5. Lo que este plan NO promete

- No promete que la fórmula del warp sea bezier: eso se decide midiendo en el paso 3.
- No promete un plazo corto: `docs/10` lo clasificó como dificultad **alta** y sigue siéndolo. Lo que
  cambia hoy es que el bloqueador tiene nombre, datos y un juez (el oráculo) en vez de conjeturas.
- No toca el editor: el editor sigue deformando con el Core. Esto es el visor independiente.
