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

### 5.2 Por qué no cierra con una sola elección: el Core **mezcla** keyforms

Medido con `tools/rot-variantes.py` y una comprobación afín (`docs/13` §5.1 sigue):

1. En `ArtMesh102`/`106` el oráculo **no es afín** en la entrada: ajustando un afín libre sobre
   todos los vértices queda un residuo de **0,0047**. Una cadena de sólo rotaciones es afín por
   construcción, así que el Core está haciendo algo **no afín** ahí.
2. Probadas 8 variantes de fórmula (pivote, ángulo negado, reflejos antes/después, escala en el
   origen, sin reflejo) y todas las combinaciones por eslabón: nada baja de 0,0061. La fórmula no
   es el problema.
3. El orden **hoja→raíz** queda confirmado: raíz→hoja da 719/1438 de error.
4. Dato clave: la banda 7 tiene claves `[0, 1]` para `ParamEyeLOpen`, y `Rotation22`/`Rotation23`
   tienen keyforms con orígenes de `-718` y `-828` unidades — estados extremos de una animación de
   parpadeo, no poses de reposo.

**Hipótesis que queda (y es la de Cubism):** el Core no elige un keyform, **interpola entre los
dos keyforms vecinos** según el valor del parámetro (peso lineal, y al cruzar parámetros el índice
es el producto mixto). Eso explica las tres cosas a la vez: el residuo no afín de estas dos mallas,
que la «excepción» de la banda 7 caiga en el índice 1 (está entre dos claves), y el error pequeño
y sistemático que arrastran las cadenas largas.

Un primer intento de mezcla lineal de `ang/origen/escala` (mismo script, función `pesos`) dio el
mismo 0,078: falta comprobar **qué** se mezcla (¿parámetros, o matrices? ¿peso por tramo de claves
o por valor normalizado del parámetro?) y con qué regla se combinan varios parámetros en un mismo
deformador. Ese es el siguiente experimento, y es el bueno: si la mezcla entra, el error de las
cadenas largas debería caer con ella.

### 5.3 La mezcla no era; el orden de las claves sí pesa (medido)

`tools/rot-mezcla.py` prueba cuatro modos de selección para las rotaciones en las **134** mallas:

| Modo | <1e-3 | <6,7e-4 (2 px) | Mediana | Máx |
|---|---|---|---|---|
| D keyform más cercano | 77 | 76 | 3,0·10⁻⁴ | 1,2·10⁻¹ |
| A parámetros mezclados | 77 | 76 | 3,0·10⁻⁴ | 1,2·10⁻¹ |
| B salidas (matrices) mezcladas | 77 | 76 | 3,0·10⁻⁴ | 1,2·10⁻¹ |
| C peso por valor normalizado | 61 | 60 | 1,4·10⁻³ | 3,6·10⁻¹ |

**La mezcla no mejora nada** (A y B dan idéntico a D) y la variante C empeora. Descartada.

Lo que sí pesa: con el **orden de claves invertido** (`k → n_kf−1−k`) las dos mallas problemáticas
bajan de 0,078 y 0,092 a **0,0081** y **0,0064** — casi exactamente el mínimo que daba la búsqueda
exhaustiva de combinaciones (0,0065 y 0,0063). O sea:

- El error dominante de ese par **es la convención de orden** de los keyforms en su banda
  (`Band 7`, `ParamEyeBallX`, claves `[0,1]`, valor por defecto 0 → la regla natural da el índice 0
  y el Core se comporta como el 1).
- Aun así queda un residuo de ~0,006 que **no** se explica por la selección ni por la mezcla.

El oráculo se genera con `dump.html`, que **sí** copia los valores por defecto antes de `update()`
(verificado en el archivo), y `ParamAngleZ` tiene defecto 0 con claves `[-10, 0, 10]` — así que no
es que el Core esté en otra pose.

**Siguiente hilo, con dos candidatos concretos:** (a) que el orden de keyforms por banda no sea
«orden de archivo» sino que haya que deducirlo de otro campo, y (b) que el residuo de ~0,006 venga
del `reflect`/escala de un eslabón concreto — se puede aislar igual que se aisló el warp.

### 5.4 La mezcla queda descartada también para la malla (medido)

`tools/pose-mezcla.py` interpola las **posiciones de la malla** entre sus dos keyforms vecinos
(que era la explicación natural del residuo no afín) y combina con rotaciones mezcladas:

| Variante | < 1e-3 | < 2 px | Mediana | Máx |
|---|---|---|---|---|
| malla sin mezcla + rotación sin mezcla | **77** | 76 | 3,0e-4 | 1,2e-1 |
| malla MEZCLADA + rotación sin mezcla | 30 | 30 | 9,7e-3 | 1,2e-1 |
| malla MEZCLADA + rotación MEZCLADA | 30 | 30 | 9,7e-3 | 1,2e-1 |
| malla sin mezcla + rotación MEZCLADA | 77 | 76 | 3,0e-4 | 1,2e-1 |

Mezclar la malla **empeora 2,5×**. La hipótesis de la interpolación de keyforms queda descartada en
los dos sitios donde podía estar (rotaciones y malla), con números. En `ArtMesh102` el error es
idéntico bit a bit en las cuatro variantes: su banda no aporta peso, así que su residuo de 0,078
viene de otro sitio.

### 5.5 Cómo se cierra esto: oráculos con parámetros movidos

El oráculo que se está usando es **una sola pose** (la de reposo), y eso pone un techo: con una
pose no se puede distinguir «eligió el keyform 1» de «mezcló 0,3 del keyform 2».

`models/samples/dump.html` acepta **overrides por URL** (`?p=ParamAngleZ=10,ParamEyeBallX=1`) y
copia los valores por defecto antes de actualizar. Generar oráculos en 3–5 valores por parámetro
—empezando por `ParamAngleZ`, `ParamEyeBallX` y los de las bandas de `Rotation21/22/23`— convierte
la selección de keyform en algo **medible directamente**: se ve qué keyform (o qué mezcla) usa el
Core para cada valor, sin inferirlo. Ese es el siguiente paso, y es barato: el navegador y el
`http.server` ya están probados en este repo.

### 5.6 Media de la selección, sobre cualquier pose (herramienta nueva)

`tools/keyform-medir.py` recorre **todas** las combinaciones de keyform de cada cadena corta
contra un oráculo cualquiera y compara la mejor combinación con la que predice la regla.

Primera corrida (cola de 14 cadenas de hasta 4 deformadores, oráculo de reposo):

```
medidas: 14   la regla coincide con la mejor combinación en 11
  ArtMesh101  (1,0,1)   vs (1,1,2)     0,0138 -> 0,0091
  ArtMesh102  (0,0,1)   vs (1,1,2)     0,0783 -> 0,0065
  ArtMesh106  (0,0,0,1) vs (1,1,1,0)   0,0921 -> 0,0063
```

Los tres desacuerdos son exactamente los tres conocidos, y en los tres la regla se queda con el
índice **0** donde el Core se comporta como el **1**. Es un solo patrón, no tres casualidades.

Y ya hay oráculos en otras poses (generados con `dump.html?p=…`, verificados: 93 mallas cambian
con `ParamAngleZ=±10`, 8 con `ParamEyeBallX=1`). Eso permite medir la regla **en varias poses** y
no sólo en reposo, que es donde estaba el techo de todos los experimentos anteriores.

### 5.7 Medido en tres poses: la regla falla igual, y el residuo es de otra cosa

Con `tools/keyform-medir.py --pose …` contra los tres oráculos (reposo, `ParamAngleZ=10`,
`ParamEyeBallX=1`), sobre las 14 cadenas cortas medibles:

| Oráculo | Regla = mejor |
|---|---|
| reposo | 11/14 |
| `ParamAngleZ=10` | **1/14** |
| `ParamEyeBallX=1` | 2/14 |

Dos conclusiones que no se podían sacar con una sola pose:

1. **La regla de selección no se sostiene al mover el parámetro.** En `ParamAngleZ=10`, para el
   deformador cuya banda tiene claves `[-10, 0, 10]`, la regla predice el índice **2** y el Core se
   comporta como el **1** (`ArtMesh103`: 0,0025 con la regla frente a **0,00001** con el 1;
   `ArtMesh104`: 0,0025 frente a **0,00000**). O sea, el índice que gana **no cambia** al pasar de
   0 a 10 — la selección no va por donde la regla cree.
2. **El residuo de `ArtMesh101/102/106` es independiente de la pose** (0,0091 / 0,0065 / 0,0063 en
   los tres oráculos). No es selección de keyform: es un error por vértice que hay que buscar en la
   geometría de esas tres cadenas.

Y un dato que anima: `ArtMesh104` llega a **0,00000** y `ArtMesh103/105` a 0,00001 con la
combinación correcta. El modelo **puede** ser exacto; lo que está mal es cómo se elige el keyform.

### 5.8 Dos convenciones probadas y descartadas

`tools/keyform-medir.py` gana `--radix-mixto` (cálculo propio del producto de bandas) y
`--invertir` (índice al revés). Resultado en las tres poses, sobre 14 cadenas cortas:

| Convención | reposo | AngleZ=10 | EyeBallX=1 |
|---|---|---|---|
| regla de `Bindings` (la de siempre) | **11/14** | 1/14 | 2/14 |
| producto de bandas calculado a mano | 1/14 | 1/14 | 2/14 |
| índice invertido | 2/14 | 2/14 | 1/14 |

- El producto calculado a mano **empeora** (1/14): el orden de significancia de las bandas
  multi-parámetro no es el que usé. La regla de `Bindings` sigue siendo la mejor conocida.
- Invertir el índice no es una regla general: ayuda a las tres mallas de reposo y rompe otras.

Queda por tanto **una sola cosa por descubrir**: cómo se ordenan los keyforms dentro de una banda
cuando hay más de un parámetro. Es el único punto que separa 77/134 de la exactitud, y ahora hay
con qué medirlo: tres oráculos, `keyform-medir.py` con búsqueda exhaustiva y 14 cadenas cortas de
banco de pruebas.

### 5.9 El modelo reproduce dos poses y **no** la tercera (medido)

Barrido de cuatro convenciones de índice (segmento, cota superior, cota superior por la derecha,
más cercano) × tres oráculos, recalculando también el keyform **de la malla** según la pose:

| Convención | reposo | `AngleZ=10` | `EyeBallX=1` |
|---|---|---|---|
| cota superior (`searchsorted left`) | 67/134 | **3/134** | 70/134 |
| más cercano | = | = | = |
| segmento / cota derecha | 2/14 en cadenas cortas | 2/14 | 2/14 |

Dos hechos, los dos útiles:

1. **`AngleZ=10` rompe el modelo entero** (3/134), mientras `EyeBallX=1` se comporta como el
   reposo (70/134). El problema no es «el modelo está mal en general»: hay un mecanismo que se
   dispara **específicamente** con `ParamAngleZ` a 10. Es el sitio exacto donde mirar.
2. La convención de índice **no** es la que explica la diferencia: con cualquiera de las cuatro,
   `AngleZ=10` colapsa igual. Así que el fallo de esa pose viene de otra parte (selección de
   keyform de un deformador concreto, o de la malla, con ese parámetro).

Con esto, el siguiente paso ya no es «probar convenciones»: es **comparar vértice a vértice** una
malla que cambia con `AngleZ` entre las dos poses (reposo y 10) para ver qué eslabón de su cadena
se mueve de forma que el modelo no predice.

### 5.10 El delta de `AngleZ` no se reproduce: el fallo es dinámico, no de desplazamiento

Comparar posiciones absolutas entre poses arrastra el error de base (0,06–0,12 en las cadenas
largas), que es **mayor** que el movimiento que produce el parámetro (0,013). Así que la
comparación útil es el **delta**: `modelo(AngleZ=10) − modelo(reposo)` contra
`oráculo(10) − oráculo(reposo)`. El offset constante se cancela solo.

Resultado sobre las **93** mallas que se mueven con `AngleZ`:

| Error del delta / movimiento | Mallas |
|---|---|
| < 10 % | **0/93** |
| < 30 % | 2/93 |
| mejor caso | 23 % (`ArtMesh27`) |
| peor caso | 505 % (`ArtMesh67`) |

Y un dato que ordena el problema: **todas** las mallas que responden a `AngleZ` tienen cadenas de
**6 o más** deformadores (histograma: 5→2, 6→48, 7→12, 8→15, 9→4, 10→4, 11→4, 12→4). Ninguna
cadena corta responde a ese parámetro.

Conclusión: el colapso de `AngleZ=10` (3/134) **no era un fallo aparte** — es que esas mallas ya
venían mal en reposo, y el delta confirma que el error es de *respuesta*, no de posición: la
selección de keyform de algún eslabón de esas cadenas largas no cambia cuando debe.

Ese es el objetivo único que queda: una cadena de 6 eslabones que responde a `AngleZ`, con el delta
como juez — que no arrastra el error de base.

### 5.11 El caso concreto, con números: el modelo se mueve la mitad

`ArtMesh63`, cadena `Warp37 → Rotation1 → Rotation2 → Warp → Warp0 → Warp1`. En reposo, la
combinación `(4, 1, 1, 0, 1, 1)` reproduce el Core con error **6·10⁻⁶**: la cadena está bien montada.

`Rotation2` es la que responde a `ParamAngleZ` (banda `[-10, 0, 10]`). Sus tres keyforms **no
rotan**: ángulo 0 en las tres, misma `esc` = 1/2976 y mismo origen en y. Lo único que cambia es
`origin_x`:

| keyform | origin_x |
|---|---|
| 0 (−10) | 0,48671 |
| 1 (0) | 0,49419 |
| 2 (+10) | 0,50088 |

Medido contra el oráculo de `AngleZ=10`:

| Estado de Rotation2 | Error contra la pose a 10 |
|---|---|
| kf 0 | 0,02076 |
| kf 1 (el de reposo) | 0,01328 |
| **kf 2** | **0,00722** |
| mezcla kf1→kf2 a 0,75 | 0,00826 |

Con kf2 el error baja pero **no llega a cero**, y el barrido de ángulo (de −2° a +2°) no lo mejora
(error mínimo 0,0046 a 1,99°). Dato que apunta a la explicación: el desplazamiento del oráculo es
**0,0133** y el que produce el paso kf1→kf2 es **0,0067** — exactamente **la mitad**. El Core se
mueve el doble de lo que explica un solo paso de keyform.

Hipótesis viva y concreta para retomar: que para ese parámetro haya **dos** pasos acumulados
(extrapolación más allá de la última clave, o bien el mismo valor aplicado al deformador y a la
malla), o bien que el índice correcto sea 2 con extrapolación desde kf1 (0,4942 + 2·0,0067 = 0,5076).
Las dos son medibles con el mismo montaje: el delta entre poses cancela todo lo estático.

### 5.12 El residuo de 0,0072 **satura**: son puntos clampeados, no un paso de keyform

Probada la extrapolación en `Rotation2` (peso del tramo kf1→kf2 de 0 a 2,25):

| peso | error |
|---|---|
| 0 | 0,013278 |
| 0,5 | 0,009931 |
| 1,0 | 0,007224 |
| 1,25 → 2,25 | **0,007224 (idéntico)** |

El error **deja de bajar y se queda clavado** en 0,007224 por mucho que se mueva el keyform. Eso no
es un problema de selección: es la firma de **puntos que se salen de la rejilla y se recortan**
(`bilineal` hace `clip` a [0, 5]). El warp deja de moverlos y el error se congela.

Consecuencia: el dominio de los warps profundos (`Warp`, `Warp0`, `Warp1`) probablemente es **más
grande** que el que se calibró, y la calibración lo compensó en reposo. Al mover el parámetro, los
puntos se salen y el clip los mata. Eso explicaría, de una vez, por qué fallan **las cadenas
largas** (6-12 eslabones) y no las cortas.

**Siguiente paso, ya concreto:** medir cuántos vértices se salen del dominio en esas cadenas
(contar `uv` fuera de [0,5] con y sin clip) y reabrir la calibración de los warps profundos como
caja libre en vez de rectángulo rígido.

### 5.13 **§5.12 queda en duda: dos corridas no coinciden** (no usar como hecho)

Al comprobar el recorte de §5.12 con un script aparte, la conclusión **no se sostiene**:

- Con `w = 1,0 / 1,5 / 2,0`, **ningún** vértice de `ArtMesh63` sale del dominio en ninguno de los
  cuatro warps (0/33 en los cuatro) y la salida **sí** se mueve (`bbox x` de `+0.0558` a `+0.0625`).
- Pero la corrida de §5.12 daba error idéntico (0,007224) para todo `w ≥ 1`, como si no se moviera.

Las dos no pueden ser ciertas: **una de las dos tiene un error de implementación** y no se resolvió
cuál. Por eso §5.12 queda marcada como dudosa y no debe usarse como punto de partida.

Lo que sí está firme de todo este tramo: el montaje de la cadena es correcto (6·10⁻⁶ en reposo con
la combinación `(4,1,1,0,1,1)`), `Rotation2` solo traslada (`origin_x` 0,48671 → 0,50088), y el
oráculo se mueve 0,0133 con `AngleZ=10` frente a 0,0067 de un paso de keyform.

**Siguiente paso real:** reescribir el evaluador de `ArtMesh63` **una sola vez**, en un script
único que sirva para las dos comprobaciones (error y conteo de fuera-de-dominio), y volver a medir
desde ahí. Con dos implementaciones distintas de lo mismo no se puede concluir nada.

### 5.14 Evaluador único: `tools/deform-eval.py` (§5.12 queda descartada del todo)

Se unificó la lógica en **un solo** archivo, con autocomprobación: en reposo, `ArtMesh63` con la
combinación `(4,1,1,0,1,1)` da **6,3·10⁻⁶**. Si no diera eso, el error estaría en el evaluador y no
en el modelo. Da eso, así que el evaluador es de fiar.

Con ese único evaluador:

| Medición | Resultado |
|---|---|
| reposo, combo conocido | 6,3·10⁻⁶ (exacto) — evaluador validado |
| `AngleZ=10`, mismo combo | 0,01328 |
| barrido de peso kf1→kf2 | 0,01328 → 0,00722 (w=1) y **ahí se queda** |
| vértices fuera del dominio, cualquier w | **0 en los cuatro warps** |

Dos conclusiones:

1. **§5.12 queda descartada**: no hay recorte. La saturación es real pero no la causa el clip.
2. La saturación significa que **el error máximo lo pone otro vértice**, uno cuyo error no depende
   del peso de `Rotation2`: hay **más de un eslabón mal** en esa cadena al mover el parámetro. La
   búsqueda exhaustiva ya lo insinuaba: el mejor combo en `AngleZ=10` era `(8, 0, 2, 1, 2, 2)`
   —cambian cinco eslabones, no uno— con error 0,0031.

**Siguiente paso, ahora sí preciso:** un script que, con este evaluador único, busque el mejor combo
en `AngleZ=10` **eslabón por eslabón** (fijando los demás y viendo cuál mejora más), para separar
cuántos están mal y en qué orden. El montaje ya está validado contra el reposo, así que cualquier
diferencia es del modelo, no del instrumento.

### 5.15 El diagnóstico final: el modelo **atenúa** el efecto del parámetro

Con el evaluador único, para `ArtMesh63` en `AngleZ=10` (y `Rotation2` en kf2, el que pide su banda):

- **Los 33 vértices** están mal, con error mediano **0,00615** y máximo 0,00722. No es un vértice
  raro: es toda la malla desplazada de menos.
- El desplazamiento que produce un paso de keyform de `Rotation2` es **0,0067** y el oráculo se
  mueve **0,0133** — dos pasos. Con kf2 el modelo da uno; el residuo es exactamente el otro.
- Perturbar el rectángulo de cada warp del dominio un ±2 % no lo arregla (el error se queda en
  0,0066–0,0072 o empeora), y el barrido de peso se satura en 0,00722 aunque se extrapole a 2,5.

**Por qué se satura, que es lo que faltaba entender:** `Rotation2` produce una traslación de
0,0067 que entra al warp siguiente como coordenada relativa (`uv`): 0,0067 / 0,569 · 5 ≈ **0,059
celdas**. Un desplazamiento de la entrada de ese tamaño, pasado por una rejilla que en esa zona es
casi rígida, sale **atenuado** en la salida. Por eso duplicar el peso no duplica la respuesta: el
modelo ya no transmite el movimiento con la ganancia correcta.

Los únicos grados de libertad que quedan son los **rectángulos de dominio** de los warps, y se
calibraron para clavar la **pose de reposo** — que se clava igual con una ganancia equivocada,
porque en reposo el parámetro no se mueve.

**Siguiente paso, y es el bueno:** recalibrar los dominios de `Warp37/Warp/Warp0/Warp1` con el
**delta entre poses** como objetivo (`modelo(10) − modelo(0)` contra `oráculo(10) − oráculo(0)`), no
con la pose absoluta. El delta cancela el offset estático y mide exactamente la ganancia, que es lo
que está mal. Todo el montaje para hacerlo está en `tools/deform-eval.py` y en los tres oráculos.

### 5.16 Recalibrar los dominios **no** arregla la ganancia (resultado negativo)

`tools/deform-delta-calibrar.py` hace descenso por coordenadas sobre los 4 valores de cada
rectángulo de warp, con el **delta entre poses** como objetivo:

| | RMS del delta | delta < 1e-3 | reposo < 1e-3 |
|---|---|---|---|
| inicio (dominios calibrados en reposo) | 0,010848 | 3/134 | **77/134** |
| vuelta 1 | 0,010228 | 3/134 | 3/134 |
| vuelta 2 | 0,009646 | 3/134 | 3/134 |

El optimizador **cambia 27 valores** y gana un 11 % de RMS en el delta… a costa de **destruir por
completo** la pose de reposo (77 → 3). Es decir: con esta estructura, los rectángulos no pueden
satisfacer las dos poses a la vez.

Conclusión importante y ahorra tiempo: **el problema no es la calibración, es la estructura.** El
modelo «warp = bilineal sobre un rectángulo normalizado» tiene suficientes grados de libertad para
clavar **una** pose, y los usa para eso; no tiene los correctos para describir cómo responde al
parámetro. Recalibrar mejor no lo va a arreglar.

### 5.17 Qué haría falta (para la siguiente sesión, no es más ajuste numérico)

Hipótesis estructurales que quedan vivas, en orden de probabilidad:

1. **El espacio de entrada del warp no es `[0,1]²` normalizado por un rectángulo**, sino el que
   define su **rejilla base** (la de reposo) — lo que `docs/12` §3 ya apuntaba como el «layout del
   pool». Se descartó invirtiendo la rejilla, pero en aquella prueba el resto de la cadena usaba los
   rectángulos: hay que probarlo con la cadena entera construida sobre esa hipótesis.
2. **El warp puede no ser bilineal por celda** cuando la rejilla no es regular: las rejillas de este
   modelo son *irregulares* (se midió la regularidad por keyform en `deform-buscar.py`). Una B-spline
   sobre rejilla irregular es la otra candidata seria.
3. Consultar la **especificación de OpenL2D** del formato (citada en `docs/03`, licencia libre): dice
   cómo se interpretan las rejillas. Leer la spec no es copiar código, y la fórmula del warp es
   justo lo que debería estar ahí.

Lo que ya está descartado con números (no repetir): mezcla de keyforms (rotación y malla), pivote,
ángulo negado, reflejos, escala en el origen, orden raíz→hoja, radix calculado a mano, índice
invertido, recorte por dominio, y recalibrar los rectángulos.

### 5.18 La especificación del formato resuelve la pregunta estructural (16 nov, cierre)

Consultada la especificación **no oficial** del formato ([AyagamiDev/docs · `model/moc3_file_format.md`](https://github.com/AyagamiDev/docs/blob/main/model/moc3_file_format.md),
reversa independiente), hay tres hechos que cambian el planteamiento:

1. **El espacio de entrada del warp sí es `[0,1]²`**, «uniformly mapped to this grid, (0,0) arriba
   izquierda y (1,1) abajo derecha». O sea: la hipótesis «cuadrado unidad» era la correcta por
   especificación, y los rectángulos que la calibración sacó distintos de `[0,1]²` (`Warp1`:
   0,569×0,798 en (0,214, 0,072)) **no son un dato del formato**: son el modelo compensando otra
   cosa mal.
2. **Hay DOS algoritmos de interpolación** por warp deformer, y no son equivalentes:
   - **legacy**: la celda se parte en **dos triángulos** por la diagonal Y=X; con `x+y < 1` se usa
     `A + x·(B−A) + y·(C−A)`, si no el triángulo `BDC`;
   - **bilineal**: lerp estándar en el cuadrilátero.
3. **Cuál se usa lo dice un flag del archivo**: `bilinear_interpolation: Bool32`, uno **por warp
   deformer**, añadido en la versión 3.03 del formato (`WarpDeformer { … V3_3 { bilinear_interpolation } }`,
   que la hexpat de OpenL2D llama *Quad Source*). Nuestro `.moc3` es versión 4.0.0, así que **tiene
   ese campo**.

Y hay un cuarto hecho, para la extrapolación: «si un vértice de entrada no está dentro de 0..1, se
aplica un algoritmo de extrapolación más complejo», que expande el espacio de entrada a 5× (2
unidades por lado) con el perímetro en un **paralelogramo** definido por los 4 vértices de esquina.

**Nuestro parser no lee ese flag** (`crates/moc3/src/body.rs` no tiene ninguna sección de warp con
tipo `Bool`), y el motor asume bilineal siempre. Esa es la explicación estructural que faltaba:

- Si un warp usa **legacy** (el modo por defecto), la interpolación no es bilineal y un ajuste
  bilineal **solo puede compensarlo torciendo el rectángulo** — exactamente lo que pasó con
  `Warp1` y `Warp47..50`, y exactamente por lo que la ganancia frente al parámetro sale mal.
- Como el error en reposo se puede clavar torciendo rectángulos, el ajuste parecía bueno y la
  dinámica estaba rota (§5.16).

### 5.19 El siguiente paso, ya sin interpretación

1. Añadir la sección `warp_deformer.bilinear_interpolation` (Bool32, 1 por warp) al `LAYOUT` de
   `crates/moc3/src/body.rs` **en su sitio** — las posiciones las da el SOT, así que se puede
   localizar sin riesgo comparando los offsets con la sección de `cols`.
2. Implementar las **dos** interpolaciones en `crates/pose` y elegir por el flag.
3. Recalibrar desde cero con `[0,1]²` fijo y sin rectángulos libres: si la especificación está
   bien, el reposo tiene que cuadrar sin tocar dominios.
4. Validar con el **delta** entre poses, que es el juez que no se deja engañar.

Nada de esto se ha probado todavía: es lo que hay que hacer, no lo que se hizo.

### 5.20 Probado: la interpolación **no** cambia nada en reposo (y el flag, localizado)

Localizado el candidato a flag en el archivo: la única sección de 50 entradas 0/1 con **mezcla**
está en el offset del SOT **41** (`0x00008800`): **17 warps legacy y 33 bilineales**. Las demás
candidatas (SOT 13, 14, 37, 38) son todas 1.

Probado en las 134 mallas en reposo, dominio `[0,1]²`, eligiendo interpolación por ese flag:

| Fuente de flags | < 1e-3 | mediana |
|---|---|---|
| SOT 37 / 38 / 13 / 14 (todo bilineal) | 12/134 | 0,03268 |
| **SOT 41 (17 legacy / 33 bilineal)** | **12/134** | **0,03268** |
| legacy en todos | 12/134 | 0,03268 |

**Idéntico hasta el último decimal.** Con dominio `[0,1]²`, cambiar la interpolación no mueve la
aguja: el error de 0,0327 no viene de ahí. Y los 77/134 de la calibración venían de mover los
rectángulos, no de la interpolación.

### 5.21 Dónde queda esto (cierre de la tanda)

Dos hechos que hay que reconciliar antes de seguir, y que hoy no se reconcilian:

1. La **especificación** dice que la entrada del warp es `[0,1]²` uniforme. Si eso es cierto, los
   rectángulos calibrados (0,569×0,798 para `Warp1`) no deberían existir y el reposo tendría que
   cuadrar sin tocarlos — y no cuadra (12/134).
2. Con los rectángulos calibrados, el reposo sí cuadra (77/134) pero la **dinámica** frente al
   parámetro está mal (§5.15, §5.16).

Lo que queda por resolver, en una frase: **cómo se convierte la posición de un hijo al espacio
`[0,1]²` del warp**. La spec lo da por supuesto; los números dicen que en este archivo esa
conversión no es la identidad. Candidatos, en orden: (a) invertir la rejilla **base** del propio
warp, (b) la rejilla base de su **keyform en reposo**, (c) un rectángulo que sí viene de algún campo
que aún no se lee.

Todo lo demás está medido y descartado con números en este documento. El siguiente que lo retome
empieza por ahí, y ya sabe qué NO hace falta volver a probar: catorce hipótesis.

### 5.22 El dato que ordena todo: cadena corta exacta, cadena larga imposible

Con dominio `[0,1]²` fijo (sin rectángulos) y **búsqueda exhaustiva** de keyforms:

| Malla | Eslabones | Error de la regla | Error de la **mejor** combinación |
|---|---|---|---|
| `ArtMesh104` | 3 (termina en warp) | — | **0,000005** |
| `ArtMesh63` | 6 | 0,05110 | **0,046001** (¡no baja!) |
| `ArtMesh131` | 6 | 0,04593 | **0,042329** (¡no baja!) |

Esto es el dato más informativo de todo el día:

- Una cadena de **3 eslabones** con dominio `[0,1]²` y **la regla de keyform sin tocar** da
  `0,000005`. La fórmula de rotación, la del warp y la regla de selección **son correctas**.
- Una cadena de **6 eslabones** no baja de 0,042 **ni probando todas las combinaciones de keyform**.
  Si el problema fuera la selección, la búsqueda exhaustiva lo encontraría. No lo encuentra.

Conclusión: **la composición de más de ~3 eslabones está mal**, y no por los keyforms. Lo más
probable es lo que quedó abierto en §5.21: después de un warp, su salida está en el espacio del
padre, y el siguiente deformador espera su entrada en el `[0,1]²` **suyo**. Esa conversión entre
eslabones es la que falta.

**Siguiente paso, ya sin ambigüedad:** para una cadena de 6, tomar los dos primeros eslabones y
calcular, con el oráculo, qué conversión entre ambos reproduce el resultado; luego con los tres
primeros, y así. Ahí se ve la regla de conversión en vez de suponerla. Es el mismo método que
funcionó con el warp aislado (`tools/warp-aislado.py`), aplicado a la frontera entre eslabones.

### 5.23 ¿Hasta dónde compone bien? Hasta 4 eslabones. Se rompe en el 5.º

Con dominio `[0,1]²` fijo y búsqueda **exhaustiva** de keyforms, agrupado por número de eslabones
(sólo cadenas con ≤1 200 combinaciones, para que la búsqueda sea completa):

| Eslabones | Exactas (< 1e-4) | Total | Peor error |
|---|---|---|---|
| 0 (sin deformador) | 1 | 1 | 0,00000 |
| 3 | **4** | **4** | 0,00001 |
| 4 | **9** | **9** | 0,00009 |
| 5 | **1** | 3 | **0,01057** |

Esto acota el problema con precisión que antes no había:

- **La composición es correcta hasta 4 eslabones.** No es «el modelo está mal»: rotación, warp,
  dominio `[0,1]²` y selección de keyform funcionan, y 13 mallas de 3 y 4 eslabones salen exactas
  sin tocar nada.
- **Se rompe en el 5.º.** A partir de ahí, ni la búsqueda exhaustiva encuentra combinación.

Y el patrón estructural que hay que mirar: las cadenas de 5 o más son las primeras que contienen un
**warp cuyo padre es otro warp**. Hasta 4 eslabones, cada warp recibe la salida de una rotación o de
la malla (coordenadas ya en `[0,1]²`); del 5.º en adelante hay que convertir la salida de un warp
—que está en el espacio del padre— a la entrada `[0,1]²` del siguiente.

**Siguiente paso, quirúrgico:** tomar una de las dos cadenas de 5 que fallan, quedarse con el tramo
«warp → warp» y ajustar con el oráculo la conversión de esa frontera (una afinidad de 6 parámetros).
Es un sistema lineal con decenas de ecuaciones y 6 incógnitas: si tiene solución exacta, la regla
está ahí y se lee en los coeficientes.

### 5.24 La hipótesis «warp→warp» era falsa: cinco warps seguidos componen exactos

Probadas las tres cadenas de 5 eslabones, todas terminadas en `Warp → Warp0 → Warp1`:

| Malla | Cadena | Mejor error (dominio `[0,1]²`, sin rectángulos) |
|---|---|---|
| `ArtMesh100` | **W W W W W** (`Warp51 → Warp46 → Warp → Warp0 → Warp1`) | **0,00000** (exacta) |
| `ArtMesh67` | W R W W W (`Warp40 → Rotation2 → …`) | 0,01057 |
| `ArtMesh43` | R R W W W (`Rotation1 → Rotation2 → …`) | falla |

- **Cinco warps consecutivos componen exacto.** La frontera warp→warp no necesita conversión: la
  hipótesis de §5.23 queda descartada con una medición.
- Las dos que fallan comparten **`Rotation2` en medio**. Y no es que `Rotation2` esté mal: en la
  cadena de 6 de `ArtMesh63` funciona bien (§5.22). Lo que falla es `Rotation2` cuando su entrada
  viene de un **warp** (`ArtMesh67`: `Warp40 → Rotation2`) o cuando va en medio de dos warps.

Ajustar una escala+traslación (2 parámetros de escala, 2 de desplazamiento) en esa frontera **no
mejora nada** (0,010568 → 0,010568, y la escala se queda en 1,003). No es una afinidad: es otra cosa.

**El dato que orienta:** `Rotation2` tiene `scale = 0,000336 = 1/2976` (exactamente `1/ppu`) y
`origin ≈ 0,49`. Esa escala solo tiene sentido en una dirección: convierte unidades del modelo a
coordenadas normalizadas. Aplicada a un punto que **ya** viene normalizado, aplasta la cadena contra
el origen — que es justo el síntoma (el resultado se vuelve casi constante y el error se queda en
~0,01).

**Siguiente paso:** determinar en qué dirección se aplica esa escala según de dónde venga la entrada
—o, lo que es lo mismo, si las coordenadas de la entrada de una rotación están en el espacio del
padre y no en unidades. Es medible con el oráculo: es la misma pregunta que se resolvió para el
warp, ahora en la frontera inversa (warp → rotación).

### 5.25 Aislada la rotación entre warps: la fórmula es exacta, el desfase es del warp

Para `ArtMesh67` (`Warp40 → Rotation2 → Warp → Warp0 → Warp1`) se aisló `Rotation2` invirtiendo
numéricamente el sufijo de warps sobre el oráculo. Después se ajustó la afinidad de su entrada a la
salida requerida:

| | Valor |
|---|---|
| Traslación ajustada | **(0,494185, 0,314391)** |
| `origin` del keyform | **(0,494187, 0,314392)** |
| Residuo del ajuste | **8,9·10⁻⁶** |
| Matriz ajustada | `diag(0,000591, 0,000421)` |
| `scale` del keyform | 0,000336 |

Dos conclusiones firmes:

1. **La fórmula de rotación es exacta.** La traslación del ajuste coincide con el `origin` del
   keyform al sexto decimal y el residuo es 9·10⁻⁶: `out = scale·p + origin` describe lo que hizo el
   Core. No hay pivote, ni signo, ni reflexión escondidos.
2. **El desfase es por eje y viene del warp.** La escala ajustada supera la del keyform en factores
   **1,759 (x)** y **1,253 (y)** — que son los recíprocos de los dos lados del rectángulo calibrado
   de `Warp1` (1/0,5689 y 1/0,798). O sea: mi inversión del sufijo estaba mal exactamente por el
   dominio de `Warp1`, no por la rotación.

Y el intento directo de derivar ese dominio como **caja de la rejilla elegida** falla en grande
(3/134, mediana 0,667): no es la caja de la rejilla.

**Con esto, lo que queda abierto es una sola cosa y está bien delimitada:** de dónde sale el
rectángulo de dominio de `Warp1` (y de `Warp47..50`), que la calibración mide como 0,569×0,798 en
(0,214, 0,072) y 1,7576×1,2529 respectivamente. Todas las demás piezas del motor están verificadas
por separado: rotación exacta, warp exacto en 4 eslabones, cinco warps encadenados exactos,
composición hasta 4 eslabones exacta, selección de keyform 11/14 en cadenas cortas.

### 5.26 La jaula como dominio se probó y **no** funciona; y las rejillas no viven en el mismo espacio (17 nov)

`tools/deform-ffd.py` implementa la lectura de *free-form deformation* que faltaba por probar con la
cadena entera: el warp no normaliza por un rectángulo, **invierte su propia rejilla neutra** (el
keyform de la pose por defecto) para obtener `(u,v)` y luego evalúa con el keyform seleccionado.
Resultado con la combinación de la regla, en las 133 mallas medibles:

| Hipótesis de dominio | Mallas < 1e-3 | Mediana |
|---|---|---|
| `[0,1]²` (`u = 5p`, lo de siempre) | 12/133 | 3,3·10⁻² |
| **invertir la rejilla neutra (FFD)** | **0/133** | **6,7·10⁻¹** |

Descartada, y con un dato que ordena el problema: **ninguna de las 50 jaulas es el cuadrado unidad**
(la desviación mínima respecto de `[0,1]²` es grande en todas) y, sin embargo, `u = 5p` es exacto en
las cadenas cortas. Es decir: el dominio **no** es la rejilla neutra del propio warp.

Y el dato nuevo, medido imprimiendo las jaulas de una misma cadena (`ArtMesh63`):

| Eslabón | Bbox de la rejilla (kf de reposo) | Espacio |
|---|---|---|
| `Warp37` (hoja) | x[+86,4 +435,5] y[−810,5 −451,0] | unidades grandes |
| `Warp` | x[+0,055 +0,945] y[+0,055 +0,945] | normalizado |
| `Warp0` | x[+0,065 +0,935] y[+0,065 +0,935] | normalizado |
| `Warp1` (raíz) | x[−0,367 +0,367] y[−0,714 +0,316] | normalizado, descentrado |

Las rejillas **no comparten espacio**: unas están en unidades (×2976 respecto de las normalizadas) y
otras en espacio normalizado, y la rotación que va entre medias es la que convierte de uno a otro
(`escala = 1/ppu`). Por eso ninguna regla de «un rectángulo por warp» puede funcionar sin saber **en
qué espacio está la entrada de cada warp**, que es exactamente la pregunta abierta de §5.21 — ahora
con evidencia de por qué lleva seis sesiones sin cerrarse.

El instrumento para seguir está ya en Rust: `l2d pose --malla <id>` imprime la cadena, el keyform de
cada eslabón y el bbox tras cada paso (`docs/15`).

**Cuántas jaulas hay en cada espacio** (medido hoy sobre las 50 de Hiyori, ancho del bbox del
primer keyform): **23 en unidades** (anchos de 157 a 1427) y **27 en espacio normalizado**
(anchos de 0,18 a 1,17). No es un caso raro de dos warps: es casi la mitad del modelo. Y el patrón
de las cadenas lo acompaña: **ningún warp en unidades alimenta a otro warp**; siempre le sigue una
rotación (`Warp37 → Rotation1 → Rotation2 → …`), que es la que con `escala = 1/ppu` devuelve el
espacio normalizado. Las mallas hacen lo mismo (`ArtMesh104` está en ±268 y su padre es una
rotación; `ArtMesh63` está en 0,2–0,8 y su padre es un warp).

### 5.27 Tres hipótesis más, medidas y descartadas (17 nov, cuarta tanda)

La pista del aspecto del rectángulo parecía la buena: el dominio calibrado de `Warp1`
(0,5690 × 0,7982) tiene **exactamente** el aspecto del lienzo (`alto/ancho = 4175/2976 = 1,4029`
frente a `0,7982/0,5690 = 1,4032`). Eso sugiere que el dominio es un cuadrado en coordenadas
normalizadas del lienzo. Probado con la cadena entera:

| Hipótesis | Mallas < 1e-3 | Mediana |
|---|---|---|
| espacio modelo, `u = 5x`, `v = 5y` (lo de siempre) | **12/134** | **0,0327** |
| entrada normalizada por el lienzo (`v = 5y/H`), jaula por jaula según su aspecto | 3/134 | 0,0873 |
| entrada normalizada multiplicando `v` por el aspecto | 3/134 | 0,1453 |
| **warp inverso** (invertir la rejilla elegida, leer la celosía identidad) | 3/134 | 1,3315 |

Las tres quedan descartadas. La coincidencia del aspecto es real pero **no** significa que la
entrada se normalice por el lienzo: al aplicarlo, se rompen justo las 9 mallas que ya eran exactas.

**Y un límite del método, medido:** de los **129** casos aislables (un warp con sólo rotaciones
después), **108 son `Warp1`** y 1 es `Warp53`; los otros 20 no bajan de 1e-3 de residuo. O sea: el
dominio sólo se puede medir hoy para el warp **raíz**. Los demás warps nunca quedan con «sólo
rotaciones después», así que no hay forma de aislarlos con este montaje.

Herramientas nuevas de esta tanda: `tools/deform-espacio.py` (espacio de entrada por jaula) y
`tools/warp-dominio.py` (rectángulo medido por warp y comparación con los candidatos del archivo:
rejilla del keyform elegido, rejilla más regular, unión de keyforms, caja de los hijos — ninguno
coincide).

**Dónde queda:** 17 hipótesis descartadas con números. Lo que falta sigue siendo una sola cosa —de
dónde sale ese rectángulo— y ya está medido que **no está en el archivo** (ninguna sección de 50×4
floats) ni sale de la geometría que se puede leer. Las dos salidas posibles son: (a) conseguir la
semántica del warp de una fuente independiente y verificable, o (b) dejar el motor en su subset
medido (12/134) y construir el producto sobre el Core, como ya hace el editor. Elegir (b) no es
rendirse: es no bloquear el producto por una incógnita de ingeniería inversa.
