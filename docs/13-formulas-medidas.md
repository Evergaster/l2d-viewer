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
