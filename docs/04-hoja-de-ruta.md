# 04 — Hoja de ruta

Cada fase termina en algo que se puede *ver* o *medir*. Si no hay hito binario, la fase no cierra.

---

## F1 — Contenedor ✅ (hecha el 2026-11-13)

- [x] `moc3` parsea cabecera, canvas, conteos y tabla de secciones (v1–v5).
- [x] `model3` parsea el manifiesto.
- [x] Rasterizador CPU con UV y blend modes.
- [x] 13 pruebas verdes; `l2d demo` produce un PNG deformado.

**Hito:** `l2d inspect` describe un `.moc3` sin abrirlo con nada de Live2D.

---

## F2 — Deformación estática (el corazón)

Objetivo: dado un `.moc3` + un valor para cada parámetro, producir los vértices finales.

| Paso | Entregable | Verificación |
|---|---|---|
| 2.1 | Leer `art_meshes`: posiciones base, UVs, índices, orden | volcar la malla en un SVG/PNG con la textura encima |
| 2.2 | Leer `parameters` + `keys` + `parameter_bindings` | imprimir curvas de un parámetro |
| 2.3 | Interpolación de keyforms (lineal entre claves) | test numérico contra valores calculados a mano |
| 2.4 | Deformador **warp** (rejilla bilineal) | deformar un quad y comparar con el esperado |
| 2.5 | Deformador **rotation** (pivote + ángulo + escala) | ídem |
| 2.6 | Composición jerárquica (padre → hijo) | un modelo con 3 niveles se deforma coherentemente |
| 2.7 | Orden de dibujo, máscaras, opacidad por parte | recorte correcto en un modelo con máscaras |

**Hito:** renderizar un modelo real a PNG con un parámetro movido, y que la silueta coincida con
la del editor (comparación por diferencia de píxeles). **Este es el momento de la verdad del
proyecto.**

**Si 2.1–2.7 no llegan a paridad:** cambiar `ModelEngine` a la implementación FFI sobre el Core
oficial (`docs/02`). El renderer y el CLI no cambian.

---

## F3 — Ventana

| Tarea | Criterio |
|---|---|
| `wgpu` + `winit` | ventana en Wayland nativo (sin XWayland) |
| Texturas y atlas | el modelo se ve con sus texturas correctas |
| Blend modes de Cubism | normal, aditivo, multiplicativo (los tres que ya tiene el rasterizador) |
| Cámara | zoom y paneo con ratón, sin tearing |
| Selección de parámetros | arrastrar un slider mueve la cara en vivo |

**Hito:** `l2d ver modelo.model3.json` abre una ventana y se puede jugar con los parámetros.

---

## F4 — Animación y física

| Tarea | Criterio |
|---|---|
| `motion3.json` | reproducir un loop de idle con fade in/out |
| `exp3.json` | aplicar una expresión |
| `physics3.json` | péndulo que responde al movimiento de la cabeza |
| `pose3.json` | intercambio de brazos |
| Lip-sync | (opcional) leer amplitud de un WAV y mover la boca |

**Hito:** un idle de 10 s en bucle sin que nada explote.

---

## F5 — Salidas y utilidades

- Export PNG por frame, GIF y secuencia.
- Fondo transparente real (canal alfa correcto).
- Modo "overlay" para OBS / escritorio (capa transparente, click-through) — es lo que la gente
  realmente quiere de un visor en Linux.
- Registro de MIME para abrir `.model3.json` con doble clic.

**Hito:** usarlo como personaje de escritorio durante una sesión de trabajo completa.

---

## F6 — Editor (decisión aparte)

Un editor es otro orden de magnitud: malla editable, crear deformadores, timeline, importar PSD.
**No se planea aquí.** Primero el visor tiene que ser aburrido de lo bien que funciona.

---

## Regla de oro

Nada de empezar F3 sin F2 cerrada. Una ventana bonita que dibuja un modelo mal deformado no es un
visor: es una captura de pantalla.
