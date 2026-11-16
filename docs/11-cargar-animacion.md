# 11 — Cargar `l2d-anim/1` y el editor en el repo (2026-11-16)

Cierra las dos deudas más baratas de `docs/10`: el proyecto **ya es un repositorio git** y el
editor **ya tiene el camino de vuelta** de su propio formato de animación.

---

## 1. Repositorio

| Qué | Valor |
|---|---|
| Remoto | https://github.com/Evergaster/l2d-viewer (público) |
| `main` | rama estable: lo probado y documentado |
| `dev` | todo el desarrollo; `main` se mueve a mano cuando `dev` está estable |
| Primer commit | `1b1aae0` — 36 archivos, 13 pruebas verdes |
| No versionado | `models/` (44 MB de muestras del SDK), `target/`, `out/*.png` |

El protocolo de cuarentena (`docs/01`) pide que el historial demuestre que el código nació de la
especificación. El primer commit **no** lo demuestra (el código es anterior al repo) y así queda
dicho en su mensaje; desde ahí, cualquier cosa nueva sí tiene historial.

Dos archivos que el `.gitignore` dejaba fuera por error de ubicación y ahora sí entran:

- **`editor/editor.html`** vivía en `models/samples/editor.html`, que está ignorado entero. La pieza
  más valiosa del tramo (679 líneas de editor) no habría entrado al repo. Se movió a `editor/`.
- `tools/editor.sh` ahora sirve la **raíz** del repo (no `models/samples`) y abre
  `/editor/editor.html?m=<modelo>`.

El Core de Live2D sigue sin versionarse: el editor lo intenta primero en
`models/samples/live2dcubismcore.min.js` y, si no está, en el CDN oficial.

---

## 2. Cargar `l2d-anim/1` (el ladrillo que faltaba)

Botones nuevos en el panel de animación: **Cargar .json** y **Borrar clave**.

La carga valida lo que dice `spec/01-formato-animacion.md` y rechaza con el motivo exacto:

| Archivo malo | Mensaje |
|---|---|
| `formato: "l2d-anim/2"` | `formato «l2d-anim/2»: esperaba l2d-anim/1` |
| claves desordenadas | `claves desordenadas: 0 después de 2` |
| dos claves con el mismo `t` | `dos claves con el mismo tiempo: 0` |
| 71 parámetros para un modelo de 70 | `el archivo declara 71 parámetros y el modelo tiene 70` |
| nombre distinto en la posición 0 | `parámetro 0 es «Otro» en el archivo y «ParamAngleX» en el modelo` |
| clave con 69 valores | `la clave en 0s trae 69 valores y hacen falta 70` |
| sin claves | `el archivo no trae claves` |

Un archivo rechazado **no toca** la animación abierta: `anim.keys` se reemplaza solo al final, ya
validado.

---

## 3. Verificado en navegador real (no a ojo)

Servidor `python3 -m http.server` sobre la raíz del repo y navegador automatizado (Chromium),
modelo **Hiyori** servido desde `models/samples`:

- Arranque: 133 mallas dibujables, 24 partes, 70 parámetros, 2 texturas, lienzo 642×900.
- Dos claves (`AngleZ` −20 en 0 s, +20 en 2 s) → 0 en 1 s y 10 en 1,5 s.
- **Botón Exportar** de verdad: la descarga llega a disco (`Hiyori.anim.json`, 70 parámetros, 2
  claves, duración 5 s).
- **Botón Cargar** de verdad, con el timeline vacío antes: el archivo entra por el `input`, deja
  `2 claves, 5s` y se recupera 0 en 1 s / 10 en 1,5 s.
- **Ida y vuelta sin pérdida:** las 140 cifras (70 × 2) del archivo y las cargadas coinciden
  exactamente.
- Borrar clave en 2 s deja 1 clave; borrar en un tiempo sin clave avisa y no hace nada.
- Los 7 archivos malos de la tabla §2 se rechazan con ese mensaje.

Captura: `docs/editor-cargar.png`.

---

## 4. Defecto encontrado por el camino

`loadImage()` usaba `await img.decode()`, y en una pestaña que no está pintando (ventana oculta o
navegador automatizado) esa promesa **no se resuelve nunca**: el editor se quedaba en «textura
texture_00.png» sin error visible. Ahora se decodifica con `createImageBitmap(blob)` —fuera del hilo
de pintado— y `img.decode()` queda como respaldo.

Para diagnosticar esto se añadió `window.__paso`, que dice en qué paso del arranque se quedó el
editor. Sirve para lo mismo la próxima vez.

---

## 5. Criterio de aceptación de `docs/10`, actualizado

| Criterio | Antes | Ahora |
|---|---|---|
| Visor independiente (error < 2 px/vértice contra el oráculo) | no | no |
| Editor: cargar / editar-borrar claves / guardar / reabrir | 1 de 4 | **3 de 4** |
| `l2d-anim/1` de ida y vuelta sin pérdida | solo ida | **ida y vuelta** |

Siguiente paso único: **guardar el modelo editado** (implica un formato propio `.l2d`, porque
escribir `.moc3` se pospuso en `docs/09`). Detrás sigue el evaluador propio en Rust.
