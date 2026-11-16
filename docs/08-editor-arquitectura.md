# 08 — Arquitectura del editor

Especificación del editor que se construye sobre el visor. Todo lo que aquí se afirma sobre el
modelo está medido con el parser propio sobre Hiyori y Wanko; lo que depende del SDK está marcado
como tal.

## 1. Qué se edita

Un modelo Live2D no es una escena 3D: es un **conjunto de mallas planas** y un **sistema de
parámetros** que las deforma. Lo medido en Hiyori:

| Elemento | Cantidad | Fuente |
|---|---|---|
| Parámetros | 24 | `art_mesh`/`parameter` del `.moc3` |
| Partes | 24 | `part` |
| Mallas de arte | 134 (129 visibles, 5 con opacidad 0) | `art_mesh` |
| Vértices | 2 822 | índices de posición |
| Triángulos | 3 366 | índices |
| Mallas con máscara | 8 | `drawable_mask` |
| Atlas de textura | 2 (`texture_00`, `texture_01`) | `model3.json` |

Editar, por tanto, es mover **vértices de mallas** y **claves de parámetros**. No hay huesos en el
sentido de un esqueleto 3D: lo que llamamos "huesos" en Live2D son **deformadores**
(warp y rotation) que transforman las mallas.

## 2. Modelo de datos en memoria

```
Documento
├── canvas: {ppu, ox, oy, ancho, alto}
├── texturas: Vec<Textura>                 (atlas, una por archivo)
├── parametros: Vec<Parametro>             (id, min, max, defecto, banda)
├── partes: Vec<Parte>                     (id, visible, orden de dibujo)
├── mallas: Vec<Malla>
│     ├── id, textura, opacidad, visible
│     ├── vertices: Vec<Vertice>           (posición, uv)
│     ├── indices: Vec<u32>                (triángulos)
│     ├── mascaras: Vec<IdMalla>
│     └── keyforms: Vec<Keyform>           (una por combinación de claves)
└── deformadores: Vec<Deformador>
      ├── Warp { rejilla: filas × columnas, keyforms }
      └── Rotation { origen, ángulo, escala, keyforms }
```

**Regla de oro:** el documento en memoria es la fuente de verdad durante la edición. El `.moc3` se
lee al abrir y se escribe al guardar. Nada de editar el archivo en sitio.

## 3. Evaluación (render de una pose)

Cadena por cada malla, en este orden exacto:

1. Elegir el **keyform** de cada malla y de cada deformador según el valor de sus parámetros
   (interpolación entre las dos claves que rodean el valor).
2. Aplicar la cadena de **deformadores** de hijo a padre, terminando en el espacio de canvas.
3. Convertir a píxeles: `px = origen + pos * ppu`, con Y hacia arriba.
4. Ordenar por **orden de dibujo** de la parte, desempatando por el de la malla.
5. Rasterizar con la **textura del atlas que le corresponde** y `v' = 1 - v`.
6. Aplicar **máscaras** (alfa de las mallas de máscara) antes de componer.
7. Componer omitiendo las mallas que son fuente de máscara y las de opacidad 0.

Los pasos 3, 5, 6 y 7 están implementados y verificados. El 1 y el 2 son lo que falta.

## 4. Módulos previstos

| Módulo | Responsabilidad | Estado |
|---|---|---|
| `moc3` | leer contenedor, cuerpo, secciones | hecho |
| `model3` | leer manifiesto | hecho |
| `raster` | triángulos, UV, máscaras, blend | hecho |
| `pose` | evaluar parámetros → keyform → deformadores | **por hacer** |
| `edit` | mutaciones del documento (mover vértice, editar clave) | por hacer |
| `io` | escribir `.moc3` de vuelta | por hacer |
| `ui` | lienzo, panel de parámetros, timeline | por hacer |
| `anim` | curvas sobre parámetros, reproducir, exportar | por hacer |

## 5. Hitos con criterio de aceptación

**H1 — Pose por parámetros (base del editor).**
Mover `ParamAngleX` de −30 a 30 mueve la cabeza y no rompe nada.
*Aceptación:* la silueta cambia de forma monótona y coherente; ninguna malla queda fuera del
canvas; comparado contra el Core, error < 2 px por vértice en la pose de reposo.

**H2 — Edición de vértices.**
Arrastrar un vértice en el lienzo lo mueve y se refleja al instante.
*Aceptación:* el vértice sigue al cursor; el resto de la malla no se deforma; undo/redo funciona.

**H3 — Claves de parámetro.**
Añadir una clave en un valor de parámetro y ver el efecto.
*Aceptación:* interpolar entre claves produce movimiento suave; el valor de reposo no cambia.

**H4 — Timeline y reproducción.**
Curvas de parámetros en el tiempo, play/pause, loop.
*Aceptación:* 24 fps estables con Hiyori completo en CPU, o 60 con GPU.

**H5 — Guardar.**
Escribir el `.moc3` editado y reabrirlo correctamente.
*Aceptación:* el archivo escrito se abre en el visor propio y las diferencias son solo las
editadas.

## 6. Restricciones que condicionan el diseño

- **Licencia.** El EULA del editor prohíbe portar o modificar su software; por eso todo esto es
  código propio. Los modelos de muestra son de Live2D y no se versionan en el repo.
- **Sin `Live2D` en el nombre del producto** (marca registrada).
- **Dependencias mínimas**: `serde`, `serde_json`, `png`. Un `winit + wgpu` entra cuando el
  rasterizador CPU deje de dar los fps; hasta entonces, CPU para poder comparar píxel a píxel
  contra el oráculo.
- **Determinismo**: mismo modelo y mismos parámetros deben dar exactamente el mismo PNG. Es lo que
  permite comparar contra el Core.

## 7. Bloqueo actual, con nombre

`keyform_position.xys` es un pool compartido entre mallas de arte y rejillas de deformadores warp.
Buscar el bloque correcto por coincidencia exacta con el oráculo no funciona (error mínimo 20–100 px
en 9 mallas probadas), porque el oráculo devuelve la geometría **ya deformada**. Para avanzar en
H1 hay que implementar warp/rotation, y para eso hace falta la especificación del SDK o un modelo
de prueba con un único keyform por malla.
