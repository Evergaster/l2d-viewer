# 14 — Guardar el modelo editado (`l2d-model/1`)

Cierra el cuarto punto del criterio del editor (`docs/10` §8): **guardar el modelo y reabrirlo con
solo los cambios aplicados**. Números medidos hoy, 17 nov 2026, en esta máquina y en Chromium real.

---

## 1. Qué se hizo

| Pieza | Archivo | Qué hace |
|---|---|---|
| Formato | `spec/02-formato-modelo.md` | `l2d-model/1`: JSON con parámetros fuera de reposo, partes cambiadas y desplazamientos de vértices |
| Lector/validador en Rust | `crates/edit/src/lib.rs` (nuevo, 8 pruebas) | huella FNV-1a 64, validación contra el `.moc3`, `aplicar()` sobre la geometría de reposo |
| CLI | `crates/l2d/src/main.rs` → `l2d edit <archivo.l2d> <modelo.moc3>` | valida el archivo sin abrir navegador |
| Editor | `editor/editor.html` → **Guardar .l2d** / **Abrir .l2d** | exporta el documento y lo reaplica sobre el modelo |

`cargo test`: **24 pruebas + 1 doctest, todas verdes** (8 de `edit`, 5 de `moc3`, 3 de `model3`,
4 de `pose`, 4 de `raster`).

El `.moc3` **no se reescribe**: el archivo de cambios es una capa encima, como estaba decidido en
`docs/09`. Un proyecto son dos archivos independientes (`l2d-model/1` y `l2d-anim/1`).

---

## 2. Por qué la huella y no un hash criptográfico

Los índices de vértice solo significan algo respecto a **un** archivo: `ArtMesh66` de Hiyori no es
`ArtMesh66` de Wanko. Por eso el documento declara `origen.bytes` y `origen.huella`, y la carga se
niega si no coinciden.

`huella` es **FNV-1a de 64 bits** sobre todos los bytes del `.moc3`, en 16 cifras hexadecimales.
Se eligió porque son diez líneas idénticas en Rust y en JavaScript, sin dependencias, y basta para
distinguir dos modelos; no pretende ser una firma. Que las dos implementaciones dan lo mismo se
verificó con archivos reales, no por confianza:

| Modelo | Bytes | Huella en el editor (JS) | Huella en `l2d edit` (Rust) |
|---|---|---|---|
| Hiyori | 443 648 | `f7d309412e4f7038` | `f7d309412e4f7038` |
| Wanko | 107 456 | `7144b685d28e48aa` | `7144b685d28e48aa` |

Además hay una prueba con los vectores publicados del algoritmo: `huella(b"") = cbf29ce484222325`,
`huella(b"a") = af63dc4c8601ec8c`, `huella(b"foobar") = 85944171f73967e8`.

---

## 3. Verificación en navegador real (Chromium, `http.server` sobre la raíz)

Escenario: Hiyori recién abierto, tres cambios hechos **con la UI** (no por consola):

1. slider real de `ParamAngleZ` con evento `input` → **15**
2. casilla real de la parte `PartEye` con `click()` → oculta (ocultas 1 → 2)
3. arrastre real: `pointerdown` / `pointermove` (+24, −18 px) / `pointerup` sobre el vértice 0 de
   `ArtMesh66` → desplazamiento **(0.037410, −0.028058)** unidades de modelo

El documento guardado contiene **exactamente esos tres cambios** y nada más:

```json
{ "formato": "l2d-model/1", "modelo": "Hiyori",
  "origen": { "archivo": "Hiyori.moc3", "bytes": 443648, "huella": "f7d309412e4f7038" },
  "parametros": { "ParamAngleZ": 15 },
  "partes": { "PartEye": false },
  "vertices": [ { "malla": "ArtMesh66", "vertice": 0, "dx": 0.03741, "dy": -0.028058 } ] }
```

| Comprobación | Resultado |
|---|---|
| Botón **Guardar .l2d** descarga de verdad | `Wanko.l2d`, 207 B, documento válido con la huella de Wanko |
| **Abrir .l2d** por el `input` real (`DOM.setFileInputFiles`) | «abierto Hiyori.l2d: 1 parámetros, 1 partes, 1 vértices» |
| Mismos parámetros tras reabrir | 72/72 idénticos |
| Mismas partes ocultas | `[3, 13]` = `PartEye` + `PartArmB` |
| Mismos desplazamientos | iguales con la precisión guardada (6 decimales; el original era `0.037410394…`) |
| **Mismos píxeles** (huella del framebuffer 642×900) | `73849980` antes de guardar y `73849980` después de reabrir |
| Aplicar el mismo archivo dos veces | idempotente: vuelve `73849980` |
| Archivo sin cambios | devuelve el reposo: `5920e1f4`, ocultas `[13]`, 0 desplazamientos |
| `l2d edit` de Rust sobre el archivo del editor | huella coincide, 1 parámetro, 1 parte, 1 vértice, «aplicado sin error» |

Rechazos probados, **todos con el estado intacto** (se valida entero antes de tocar el modelo):

| Archivo mutado | Mensaje |
|---|---|
| huella de otro modelo | `huella «0000000000000000» en el archivo, «f7d309412e4f7038» en Hiyori.moc3: es otro modelo` |
| `bytes` cambiados | `el archivo declara 123 bytes y Hiyori.moc3 tiene 443648` |
| sin `origen` | `el archivo no declara el .moc3 de origen (origen.huella)` |
| `formato: l2d-model/2` | `formato «l2d-model/2»: esperaba l2d-model/1` |
| malla inexistente | `malla «ArtMesh9999» no existe en el modelo` |
| vértice fuera de rango | `«ArtMesh66» tiene 30 vértices y el archivo mueve el 30` |
| parámetro inexistente | `parámetro «ParamInexistente» no existe en el modelo` |
| parte inexistente | `parte «PartCola» no existe en el modelo` |
| `.l2d` de Hiyori abierto en Wanko | `el archivo declara 443648 bytes y Wanko.moc3 tiene 107456` |

---

## 4. Defecto encontrado (y corregido en el camino)

**Síntoma:** aplicar un documento **vacío** no devolvía el modelo al reposo; dejaba la edición
anterior puesta (huella de píxeles `2c99d455` en vez de `5920e1f4`).

**Causa:** la primera versión aplicaba solo lo que el archivo listaba, como un parche incremental.
Con un archivo sin claves, no había nada que aplicar y el estado previo seguía vivo.

**Arreglo:** cargar un documento **restablece primero el estado base** (parámetros por defecto,
visibilidad de arranque con `PartArmB` oculta, cero desplazamientos) y encima aplica sus cambios.
Con eso el archivo describe el estado editado completo, cargarlo dos veces da lo mismo y el vacío
significa «modelo intacto». `spec/02` lo dice explícitamente para que nadie lo vuelva a invertir.

---

## 5. Límite conocido, no defecto

El desplazamiento se suma a la posición que entrega el **Core** con los parámetros guardados: vive en
el espacio del Core, no en el local del deformador. Si después se mueve un parámetro, el vértice
editado viaja con la deformación en vez de quedarse fijo respecto a su malla. Quitarlo exige el rig
propio (`docs/12`), no un cambio de formato: `malla` + `vertice` son los campos que usará el rig.

---

## 6. Cómo retomar

```bash
cd ~/Proyectos/l2d-viewer && git switch dev

cargo test                                # 24 pruebas + 1 doctest
tools/editor.sh                           # Hiyori; editor/editor.html?m=Wanko para el pequeño
cargo run -p l2d -- edit ~/modelo.l2d models/samples/Samples/Resources/Hiyori/Hiyori.moc3
```

El editor se sirve desde la raíz del repo: `python3 -m http.server 8771 --bind 127.0.0.1` y abrir
`http://127.0.0.1:8771/editor/editor.html?m=Hiyori`.

Ganchos de verificación que quedaron en la página (solo para pruebas, no se usan en la UI):
`window.__documentoModelo()`, `window.__aplicarModelo(doc)`, `window.__huellaPixeles()` y
`window.__estado()`.

---

## 7. Criterio de aceptación (`docs/10` §8) — ahora

- **Editor completo:** cargar lo exportado ✅, editar y borrar claves ✅ (`docs/11`), guardar el
  modelo ✅ y reabrirlo con solo los cambios aplicados ✅ → **4 de 4**.
- **Visor independiente:** sigue sin cumplirse; el evaluador de deformadores es el bloqueador y su
  estado medido está en `docs/13`.
- **Animación:** `l2d-anim/1` de ida y vuelta sin pérdida ✅.

El siguiente paso único ya no es guardar: es el **evaluador propio en Rust** (`crates/pose`: elegir
keyform por valor de parámetro, luego warp y rotation), con el límite de §5 como recordatorio de por
qué la edición manual todavía no es un rig.
