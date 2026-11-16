# 03 — El formato `.moc3` (contenedor)

Lo que este repo implementa hoy, y con qué fuente.

## Fuente

Dos fuentes, y la segunda manda:

1. **OpenL2D, «MOC3 Format Specification 2.1b»** (`github.com/OpenL2D/moc3ingbird`,
   `src/moc3.hexpat`) — descripción pública del formato.
2. **Los 8 modelos de muestra oficiales del SDK** (`Live2D/CubismWebSamples`), que se usaron
   para **verificar** la especificación.

Resultado de verificar: **la especificación comunitaria se equivoca en dos puntos del
contenedor**. Lo que sigue es lo que dicen los archivos reales, no lo que dice el documento.

| Punto | Especificación | Archivos reales (v1, v3, v5, v6) |
|---|---|---|
| Inicio de la tabla de offsets | `0x40 + 0x280` = `0x2C0` | **`0x40`**, sin relleno previo |
| Bloque de conteos | relleno de 128 B (256 en v5) | **sin relleno**: los conteos empiezan en el offset indicado |
| Orden de campos del canvas | `ppu, originX, originY, w, h` | no coincide: ver «canvas» abajo |

## Layout del archivo (verificado)

```
0x000  Header                     64 B
0x040  ├─ countInfo      : u32    → offset de los conteos
0x044  ├─ canvasInfo     : u32    → offset del bloque de canvas
0x048  └─ offsets por sección     (N × u32, N = suma de conteos, en orden fijo)
```

### Header (`0x000`, 64 B)

| Offset | Tipo | Campo |
|---:|---|---|
| 0 | `[u8;4]` | `"MOC3"` |
| 4 | `u8` | versión de formato (1–5) |
| 5 | `u8` | big endian (0 = little) |
| 6 | — | relleno hasta 64 |

| Versión | Formato | Notas |
|---:|---|---|
| 1 | 3.0.0 | base |
| 2 | 3.3.0 | + keyforms de warp v2 |
| 3 | 4.0.0 | igual que 2 en layout de contenedor |
| 4 | 4.2.0 | + colores de keyform, blend shapes, extensiones de parámetro |
| 5 | 5.0.0 | + keyforms v5, blend shapes de partes/rotation/glue |

### CanvasInfo (5 `f32` + flags) — CORREGIDO con el Core oficial

El orden es el que decía la especificación, y las "variantes" que documenté antes eran un error
mío de lectura. **Verificado con el Core oficial en 5 modelos**:

| Modelo | raw en archivo | Core dice |
|---|---|---|
| Wanko | 1200, 600, 600, 1200, 1200 | 1200×1200, origen (600,600), ppu 1200 |
| Hiyori | 2976, 1488, 2087.5, 2976, 4175 | 2976×4175, origen (1488, 2087.5), ppu 2976 |
| Rice | 3000, 2250, 1500, 4500, 3000 | 4500×3000, origen (2250,1500), ppu 3000 |
| Mark | 2400, 1200, 1500, 2400, 3000 | 2400×3000, origen (1200,1500), ppu 2400 |
| Mao | 5800, 2895, 4196, 5800, 8400 | 5800×8400, origen (2900,4200), ppu 5800 |

→ `[pixelsPerUnit, originX, originY, ancho, alto]`, y `reverse_y` es el bit 0 del byte en `+20`.

**Segundo hallazgo:** las posiciones de vértice del archivo ya están en **píxeles de canvas**;
el Core las *normaliza* dividiendo por `pixelsPerUnit` antes de exponerlas
(`csmGetDrawableVertexPositions` devuelve unidades, multiplica por ppu para volver a píxeles).

### CountInfo

Los contadores, en orden fijo, **empezando en el offset indicado** (sin relleno, a diferencia de
lo que dice la especificación):

**23 base**: parts, deformers, warp deformers, rotation deformers, art meshes, parameters,
part keyforms, warp keyforms, rotation keyforms, art-mesh keyforms, keyform positions,
parameter binding indices, keyform bindings, parameter bindings, keys, uvs, position indices,
drawable masks, draw order groups, draw order group objects, glue, glue info, glue keyforms.

**+9 (v4)**: colores multiply, colores screen, bindings de blend shapes (parámetro y keyform),
blend shapes de warp y de art mesh, índices/constraints/valores de constraints.

**+3 (v5)**: blend shapes de parts, de rotation deformers y de glue.

### Tabla de secciones

Desde `0x2C8` y en el mismo orden de la lista de arriba: por cada elemento de cada sección, un
`u32` con su offset en el archivo. Número total de `u32` = suma de todos los conteos.

Orden y secciones extra por versión (las que este repo ya recorre):

- base: 23 secciones
- v≥2: +`warp_deformer_keyforms_v2`
- v≥4: +14 (extensiones, v4 de warp/rotation/artmesh, colores, parámetros v4, blend shapes)
- v≥5: +6 (keyforms v5, blend shapes de parts/rotation/glue)

→ **23 / 24 / 24 / 38 / 44** secciones para v1…v5. Eso es exactamente lo que comprueba
`secciones_por_version`.

## Qué significa cada sección (para la fase 2)

| Sección | Contenido |
|---|---|
| `parts` | id, opacidad, índice del padre, offscreen |
| `art_meshes` | id, flags, blend mode, índice de textura, orden, máscaras, vértices, UVs, índices, colores |
| `warp_deformers` / `rotation_deformers` | rectángulo base, número de subdivisiones, keyforms |
| `*_keyforms` | posiciones de los puntos de control por combinación de parámetros |
| `parameter_bindings` | qué keyform corresponde a qué valor de qué parámetro |
| `keys` | valores de las claves de interpolación |
| `drawable_masks` | qué drawables enmascaran a cuál |
| `draw_order_groups` | grupos de orden de dibujo |
| `uvs`, `position_indices` | geometría de las mallas |

## Lo que NO está implementado (y no se finge)

- Lectura de los datos de keyform (solo se leen sus *offsets*).
- Evaluación de la deformación.
- Blend shapes y colores por keyform.
- Glue.

Eso es la fase 2 de `docs/04-hoja-de-ruta.md`. Sin un modelo real para comparar, escribir esa
parte ahora sería adivinar.

## Cómo conseguir un modelo de prueba legalmente

1. **Los de muestra oficiales del SDK**: `github.com/Live2D/CubismWebSamples` →
   `Samples/Resources/` (Haru, Hiyori, Mao, Mark, Natori, Ren, Rice, Wanko). Son públicos y su
   licencia permite usarlos para desarrollar con el SDK. Es lo que usa este repo:

   ```bash
   git clone --depth 1 --filter=blob:none --sparse \
     https://github.com/Live2D/CubismWebSamples.git models/samples
   cd models/samples && git sparse-checkout set Samples/Resources
   ```

2. **Tuyo**: cualquier modelo que hayas creado o comprado con derechos de uso.
3. **Nunca**: modelos de nizima o de terceros sin permiso, ni siquiera «para probar».

`models/` está en `.gitignore`: los modelos no se suben.

## Verificación con archivos reales (2026-11-13)

| Modelo | Versión | Tamaño | Canvas | Partes | Mallas | Parámetros |
|---|---|---:|---|---:|---:|---:|
| Wanko | 1 | 105 KiB | 1200 × 1200 | 15 | 34 | 25 |
| Haru | 1 | 376 KiB | 2400 × 4500 | 19 | 84 | 42 |
| Natori | 1 | 753 KiB | 4000 × 7000 | 32 | 176 | 96 |
| Mark | 3 | 122 KiB | 2400 × 3000 | 17 | 30 | 21 |
| Hiyori | 3 | 433 KiB | 2976 × 4175 | 24 | 134 | 70 |
| Rice | 3 | 468 KiB | 3000 × 4500 | 30 | 178 | 96 |
| Mao | 5 | 859 KiB | 5800 × 8400 | 32 | 262 | 132 |
| Ren | 6 | 894 KiB | 5200 × 7000 | 51 | 198 | 73 |

Los 8 parsean. Eso convierte el lector de «probablemente correcto» en «correcto para cuatro
versiones del formato».
