# 07 — Cierre de la fase (2026-11-13)

Documento de cierre. Qué se construyó, qué se verificó, qué **no** está hecho y por qué.
Sin adornos.

## Qué es este proyecto hoy

Un **lector del formato `.moc3` escrito en Rust desde cero**, con un rasterizador propio, que
abre modelos reales, extrae su geometría, UVs, texturas, partes, parámetros y máscaras, y los
dibuja en un PNG. No usa ni una línea de Live2D.

## Lo que está hecho y verificado

| Pieza | Estado | Verificación |
|---|---|---|
| Contenedor `.moc3` (cabecera, canvas, tabla de secciones) | ✅ | 8 modelos reales, versiones 1, 3, 5 y 6 |
| Cuerpo por secciones (SOT + tipos por campo) | ✅ | ids, conteos y offsets idénticos a `py-moc3` |
| `model3.json` | ✅ | 3 pruebas, incluido manifiesto mínimo y completo |
| Rasterizador CPU (triángulos, UV bilineal, 3 blend modes, PNG) | ✅ | 4 pruebas |
| Máscaras (malla-máscara → alfa) | ✅ | 8 mallas de Hiyori con máscara |
| Orden de dibujo por parte (`part_keyform.draw_orders` + visibilidad) | ✅ | capas correctas en Hiyori |
| CLI (`inspect`, `body`, `render`, `model3`, `demo`) | ✅ | usado sobre 8 modelos |

`cargo test`: **7 suites, 13 pruebas, todas verdes**. `cargo build --release` limpio.

## Lo que NO está hecho

| Falta | Consecuencia visible | Dificultad real |
|---|---|---|
| **Selección de keyform por parámetro** | la cara del personaje no aterriza: se dibuja con un keyform aproximado y las piezas de la cara se van | alta: hay que deshacer el índice de keyform como producto de bandas |
| `draw_order_group` | empates de orden resueltos por orden de archivo | media |
| Deformadores warp/rotation | mover un parámetro no deforma nada | alta (es el corazón del runtime) |
| `motion3.json` / física / ventana | no hay animación ni ventana | alta |

**Veredicto:** es un lector de formato sólido y un renderizador de geometría correcto, pero
**no es todavía un visor**. Falta el motor de evaluación de keyforms, que es la mitad del trabajo
de un runtime de Live2D.

## El bloqueador, con nombre y apellidos

Por malla, el archivo guarda N keyforms (poses). Para dibujar en reposo hay que elegir el keyform
cuyos valores de parámetro son los de por defecto. Los datos están ahí:

- `parameter.keyform_binding_begin/count` → rango por parámetro
- `keyform_binding_index.indices` → bandas por parámetro
- `keyform_binding_band.begin/count` → claves por banda
- `keys.values` → valores

Medido en Wanko: 25 parámetros, 21 bandas, 126 claves, y `PARAM_ANGLE_X` → banda 11 →
`[-10, 0, 40, 50]`. El problema: `keyform_count` de una malla **no** coincide con las claves de
su banda (2 de 34 mallas), así que el índice de keyform es una combinación en **radix mixto**
sobre varias bandas. Sin deshacer esa combinación, cualquier heurística (primero, central, centroide
mediano) acierta en unas mallas y falla en otras.

Probadas las tres heurísticas: `L2D_KF=primero|centro|mediana`. Ninguna resuelve la cara.
Por eso no se elige una y se dice que está bien: se documenta como lo que es, pendiente.

## Cómo continuar (para el que siga)

1. **Radix mixto.** Para cada malla, tomar sus bandas (empezando en
   `art_mesh.keyform_binding_band_indices[i]`), multiplicar sus conteos de claves, comprobar que
   el producto es `keyform_count`. Con eso, índice de keyform = suma ponderada de índices de clave.
2. Elegir por cada banda el índice cuya clave esté más cerca del `default` del parámetro.
3. Verificar: la silueta de Hiyori debe coincidir con el editor ±2 px.
4. Después: `draw_order_group`, y luego deformadores (warp primero, rotation después).

## Intento de oráculo (2026-11-13, después del cierre)

Para no seguir adivinando intenté obtener la verdad de posiciones por defecto desde el **Core
oficial para Web** (`https://cubism.live2d.com/sdk-web/cubismcore/live2dcubismcore.min.js`,
207 KB, público). Resultado:

- El archivo se descarga y **se deja cargar como módulo** en Node: `require()` devuelve
  `AlignofMoc`, `MocVersion_*`, `ParameterType_*`, `Version.csmGetVersion`…
- Pero el runtime de Emscripten no se inicializa en ese contexto: al llamar
  `Version.csmGetVersion()` falla con `Cannot read properties of undefined (reading 'apply')`,
  porque `_em.ccall` no existe todavía.
- Conclusión: ese build está pensado para navegador. Cargarlo requiere servirlo por HTTP y
  ejecutarlo en una página (o en el demo oficial), no en CommonJS.

Queda el script `tools/oraculo-core.js` y el hallazgo escrito. **El siguiente intento debe ser en
navegador**: servir `models/samples` por `http.server`, abrir una página que cargue el core, el
`.moc3` y llame a `csmGetDrawableVertexPositions`, y volcar el JSON. Con eso se cierra el radix.

## Corrección importante tras el oráculo

El oráculo del Core oficial (en navegador, ver abajo) desmintió dos cosas de este documento:

1. **El canvas** no era "sin interpretar": es `[ppu, ox, oy, ancho, alto]`, tal como decía la
   especificación. Ya está corregido en el código y en `docs/03`.
2. **El bloqueador no es (solo) la selección de keyform.** Con el canvas correcto el modelo sigue
   desarmado. La causa real es que **las posiciones de cada malla están en el espacio local de su
   deformador padre**: sin aplicar la cadena de deformadores (warp/rotation), cada pieza aparece
   donde no toca. Eso explica que Wanko (pocas mallas, deformadores casi identidad) salga
   coherente e Hiyori no.

Orden correcto de trabajo, revisado:

1. Deformadores warp y rotation (aplicar la cadena padre→hijo).
2. Después, la selección de keyform (que sigue pendiente, pero ya no es lo primero).
3. Después, `draw_order_group` y `renderOrders`.

## Oráculo del Core: cómo se usa (funciona)

`models/samples/oraculo.html` + `live2dcubismcore.min.js` servidos con `python3 -m http.server`.
En el navegador se obtiene, para cualquier modelo, la verdad del Core:

```js
const Core = window.Live2DCubismCore;
const moc = Core.Moc.fromArrayBuffer(buf);
const model = Core.Model.fromMoc(moc);   // OJO: no es moc.instantiateModel()
model.update();
model.canvasinfo;            // CanvasWidth/Height/OriginX/OriginY/PixelsPerUnit
model.drawables.vertexPositions[i];  // pose por defecto, normalizada
model.drawables.renderOrders;        // orden final ya resuelto por el Core
```

Ese es el patrón de comparación para todo lo que falta. En Node **no** funciona: el runtime de
Emscripten no se inicializa en CommonJS.

## Hipótesis abierta: ¿unidades o píxeles? (2026-11-13, final)

Dos medidas que no encajan entre sí, y conviene dejarlas escritas en vez de elegir una:

| Medición | Resultado |
|---|---|
| La única malla de Hiyori **sin** deformador padre | bbox `[-0.2, -0.4, 0.2, 0.1]` → escala de **unidades** |
| Render multiplicando por `ppu` (unidades → px) | cabeza gigante en la esquina: **no** cuadra |
| Render sin multiplicar (tratar como px) | torso fragmentado a escala razonable, pero desarmado |
| Mallas con coordenadas absurdas | **50 de 134** al interpretar como píxeles |

Conclusión honesta: hay una mezcla de dos problemas y no se pueden separar a ojo.

1. Para ~50 mallas el keyform elegido es el equivocado, y sus vértices salen disparados.
2. Las que sí caen en rango producen una silueta coherente a escala de píxeles.

El oráculo del Core resuelve esto **mecánicamente**, sin más conjeturas: volcar
`Core.drawables.vertexPositions[i] * ppu` a JSON y compararlo contra cada bloque de keyform de
cada malla. La coincidencia (error < 0,01) da a la vez la escala correcta y el índice de keyform.
Ese es el primer paso del siguiente trabajo, y está a una hora de distancia con el HTML ya hecho.

Código en este estado: compila, 13 pruebas verdes, guarda de cordura activada para no dibujar
mallas con coordenadas imposibles.

## RESUELTO: el render ya es visualmente correcto (2026-11-13)

Tres causas, encontradas midiendo contra el Core oficial:

1. **UV con V invertida.** Live2D guarda las UV con origen arriba-izquierda y el muestreo del
   atlas necesita `v' = 1 - v`. Sin eso, cada triángulo pinta otra zona del atlas y el modelo
   parece "despiezado". **Era el defecto principal.**
2. **Mallas que son fuente de máscara.** En Cubism son geometría auxiliar: recortan a otras y no
   se dibujan como arte. Dibujarlas producía un bloque blanco tapando el torso.
3. **Parámetros sin valores por defecto.** El Core arranca los parámetros en 0; hay que copiar
   `defaultValues` a `values` antes de `update()`, si no el modelo se deforma a cero.

Con las tres corregidas, Hiyori y Wanko salen completos y correctos:

```
l2d oracle models/samples/hiyori.oracle.json <tex_dir>/texture_00.png out/hiyori.png 0.5
```

## Calibración del keyform contra el oráculo (hecho)

`tools/calibrar-keyform.py` compara cada keyform de cada malla contra la geometría del Core.

- Con la tabla de offsets `art_mesh_keyform.keyform_position_begin_indices`, ambos criterios de
  escala coinciden **130/130** mallas.
- **El keyform de reposo es el 0 (el primero)**: penalización mediana 0, máxima 0.0064 unidades
  (~19 px). Los heurísticos `mediana`/`centro` estaban equivocados y ya no son el defecto.

Lo que queda del lado del parser propio es un desacuerdo de índice: `art_mesh.keyform_begin_indices`
apunta a una tabla de 1242 entradas, y en el render propio 66 mallas leen bloques con escala de
píxeles en vez de unidades — señal de que se está indexando `pb` con un índice local donde va el
global (o al revés). Con `L2D_DEBUG=1` se ven las dos escalas conviviendo en el mismo modelo.

## Lo que este cierre NO es

No es «el visor está listo». Es «la investigación y la base están cerradas y verificadas, y el
siguiente paso está identificado con precisión». El proyecto sigue abierto, pero ya no a ciegas.
