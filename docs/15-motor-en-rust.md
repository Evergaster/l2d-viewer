# 15 — El motor de deformación, en Rust (`crates/pose`)

Convierte en código lo que `docs/13` midió, y deja **medido** cuánto acierta hoy. Números de
esta máquina, 17 nov 2026.

---

## 1. Qué se implementó

| Pieza | Archivo | Qué hace |
|---|---|---|
| Selección de keyform | `crates/pose/src/seleccion.rs` | `Bindings`: `keyform_binding_*` → índice de keyform por **radix** (`docs/13` §2), con valores por defecto o de una pose concreta |
| Fórmulas | `crates/pose/src/deformar.rs` | `rotacion()` (`escala·R(ángulo+base)·p + origen`), `warp()` (bilineal por celda, `p∈[0,1]²`), `evaluar()` hoja→raíz, y `evaluar_pasos()` con el estado tras cada eslabón |
| CLI | `crates/l2d/src/main.rs` → `l2d pose` | evalúa la pose, compara contra un volcado del Core y resume aciertos |
| Marcador | `crates/pose/tests/marcador.rs` | dos pruebas que fijan el **suelo** actual, para que no empeore sin que se note |

`cargo test`: **32 pruebas + 1 doctest**, todas verdes (10 de `pose`, 8 de `edit`, 5 de `moc3`,
3 de `model3`, 4 de `raster`, 2 del marcador).

```bash
cargo run -p l2d -- pose models/samples/Samples/Resources/Hiyori/Hiyori.moc3 \
    --oraculo models/samples/hiyori.oracle.json
cargo run -p l2d -- pose …/Wanko.moc3 --oraculo models/samples/wanko.oracle.json
cargo run -p l2d -- pose …/Hiyori.moc3 --oraculo … --malla ArtMesh63   # paso a paso
cargo run -p l2d -- pose …/Hiyori.moc3 --oraculo … --param ParamAngleZ=10
```

`--malla` imprime la cadena con el keyform elegido en cada eslabón, los campos de cada
rotación (ángulo, escala, origen, reflejos) y el **bbox tras cada paso**. Es el instrumento que
faltaba para atacar lo que sigue abierto.

---

## 2. Lo que mide hoy (y coincide con la referencia en Python)

| Modelo | Exactas (< 1e-3) | Mediana | Máximo |
|---|---|---|---|
| Hiyori (134 mallas) | **12/134** | 3,30·10⁻² | 0,0921 |
| Wanko (34 mallas) | **12/34** | 2,10·10⁻¹ | 0,8437 |

Son los mismos números que dan los scripts de `docs/13` con la misma regla: **134/134 y 34/34
comparables, con el mismo conjunto exacto y el mismo máximo**. Que dos implementaciones
independientes (Python de investigación, Rust del producto) coincidan al sexto decimal es la
prueba de que las fórmulas están bien copiadas; lo que falla no es la transcripción.

Las 12 exactas de Hiyori son las de cadena corta (`HitArea`, `ArtMesh93…106` y `ArtMesh100`),
con errores de 3·10⁻⁶ a 9·10⁻⁵ — es decir, precisión de `f32`.

---

## 3. Lo que se aprendió implementándolo (tres trampas reales)

1. **El orden es hoja→raíz.** `Modelo::cadena()` devuelve la cadena de la raíz a la malla
   (cómodo para leer), y la primera versión la recorría así: el error pasó de 0,05 a **709**.
   `evaluar()` la invierte y el doc-comment lo dice para que no se repita. `docs/13` §5.2 ya lo
   tenía medido; ahora está también en el código.
2. **La métrica del error cambia el número.** El primer CLI comparaba con **distancia
   euclídea** y daba 0,0692 donde la referencia decía 0,0511. Los bbox por paso coincidían a
   cuatro decimales: no era el motor, era la métrica. Ahora se usa la mayor diferencia por
   coordenada (Chebyshev), la misma de `docs/13`, para que los números sean comparables.
3. **El keyform de la malla no es «el 1».** Varios experimentos de `docs/13` evalúan la malla
   con `kf=1` fijo; en mallas con un solo keyform eso lee el bloque de **otra** malla. Rust lo
   elige por la regla de `Bindings`; con la regla también en Python, los dos dan 12/134 y
   mediana 0,0327. La coincidencia confirma que la diferencia era esa y no el motor.

---

## 4. Lo que sigue abierto (igual que ayer, pero ahora con instrumento)

El hueco es el mismo de `docs/13` §5.21: **cómo se convierte la posición de un hijo al espacio
`[0,1]²` del warp**. El motor usa `[0,1]²` (lo que dice la especificación) y por eso acierta
las cadenas de 3–5 eslabones y falla las largas.

Con `--malla` y `evaluar_pasos()` ya se puede ver, eslabón por eslabón, en qué paso el bbox se
desvía del oráculo — que es exactamente lo que pedía el cierre de `docs/13`. Nada de esto se
resolvió hoy y no se presenta como resuelto: el motor está **honestamente a medias**, con el
número a la vista en cada corrida.

---

## 5. Criterio de aceptación (`docs/10` §8)

- **Visor independiente:** render de Hiyori sin Core con error < 2 px por vértice. Hoy: 12 de
  134 mallas (9 %). **No cumple.**
- **Editor completo:** 4 de 4 (`docs/14`).
- **Animación:** `l2d-anim/1` de ida y vuelta sin pérdida (`docs/11`).

El siguiente paso único sigue siendo el mismo del cierre de `docs/13`: derivar la conversión de
espacio en la frontera `rotación → warp` midiendo con los brazos existentes.
