# 10 — Cierre del editor (2026-11-14)

Documento de cierre del tramo **editor**. Cierra lo que `docs/07` dejó abierto como «falta el motor
de keyforms» y lo que `docs/08` planeó como hitos H1–H5.

Regla del documento: solo entra lo **verificado en esta máquina**. Lo supuesto va marcado como
supuesto. Números medidos hoy, 14 nov 2026.

---

## 1. Qué hay, con números

| Pieza | Archivo | Tamaño | Estado |
|---|---|---|---|
| Lector del contenedor `.moc3` | `crates/moc3/src/lib.rs` | 496 líneas | ✅ verificado contra 8 modelos reales (versiones 1, 3, 5, 6) |
| Cuerpo por secciones (SOT, tipos por campo) | `crates/moc3/src/body.rs` | 524 líneas | ✅ ids, conteos y offsets idénticos a `py-moc3` |
| Manifiesto `.model3.json` | `crates/model3/src/lib.rs` | 183 líneas | ✅ 3 pruebas |
| Rasterizador CPU (UV bilineal, 3 blend modes, máscaras, PNG) | `crates/raster/src/lib.rs` | 362 líneas | ✅ 4 pruebas |
| CLI `l2d` | `crates/l2d/src/main.rs` | 762 líneas | ✅ 7 subcomandos |
| Editor | `editor/editor.html` | 679 líneas | ✅ prototipo funcional (WebGL2 sobre el Core) |
| Formato de animación | `spec/01-formato-animacion.md` | — | ✅ `l2d-anim/1` documentado y usado por el exportador |

Total Rust: **2 327 líneas**. `cargo test`: **12 pruebas unitarias + 1 doctest = 13, todas verdes**
(verificado hoy con `cargo test`).

Subcomandos reales del CLI: `inspect`, `model3`, `body`, `params`, `render`, `oracle`, `demo`.

---

## 2. Qué hace el editor hoy (verificado con capturas)

Capturas en `docs/editor-*.png`, generadas sirviendo `models/samples` por HTTP.

- **Panel de parámetros**: un slider por parámetro del modelo (**72 en Hiyori** según
  `l2d params`; el README antiguo decía 70 — el editor dibuja uno por cada parámetro que declara el
  Core). Al moverlo, el modelo se deforma y se redibuja en el acto.
- **Edición de vértices**: nodos visibles sobre la malla, arrastre en vivo, **deshacer** por pila de
  estados. Los desplazamientos se guardan por `malla:vértice` y se aplican al subir los vértices a
  la GPU, así la deformación del Core y la edición del usuario no se pisan.
- **Capas**: casilla por parte (24 en Hiyori); ocultar una parte apaga todas sus mallas.
  `PartArmA`/`PartArmB` (dos juegos de brazo) se resuelven ocultando la variante B por defecto.
- **Máscaras**: recorte por esténcil con incremento por máscara y `EQUAL` al número de máscaras —
  es la intersección que usa Cubism.
- **Claves y timeline**: se fija tiempo, se mueven sliders, **Añadir clave**; **Reproducir**
  interpola lineal y hace bucle; **Exportar .json** baja el archivo en `l2d-anim/1`.
- **Prueba de interpolación**: dos claves (`AngleZ` −20 en 0 s, +20 en 2 s) dan 0 en 1 s y 10 en
  1,5 s, con reloj real.

---

## 3. Qué NO hace (y qué cuesta cada cosa)

| Falta | Consecuencia visible | Dificultad | Nota |
|---|---|---|---|
| Guardar el modelo editado | los cambios de vértices mueren al cerrar la pestaña | media-alta | implica escribir `.moc3` (hito H5) o un formato propio `.l2d` |
| **Rig propio en Rust** (`pose`, `edit`, `io`, `ui`, `anim`) | el editor depende del Core oficial para deformar | alta | `docs/09` concluye que no hace falta replicar Cubism, pero sí escribir el evaluador propio |
| Deformadores warp / rotation | el parser propio no puede componer la pose por sí solo | alta | es el corazón del runtime; `keyform_position.xys` es un pool compartido (ver §6) |
| `draw_order_group` | empates de orden resueltos por orden de archivo | media | solo afecta al render del parser propio |
| Ventana nativa | el editor vive en el navegador | media | exige `winit`/`wgpu`, fuera del alcance actual |

**Hecho después de este cierre** (16 nov, `docs/11`): **cargar** una animación exportada y
**borrar** claves. El ciclo de `l2d-anim/1` está cerrado y verificado en navegador.

**Veredicto:** hay un **editor usable** y un **lector de formato sólido**, pero siguen siendo dos
cosas separadas: el editor deforma con el Core, el parser lee sin deformar. Unirlas es el rig propio
en Rust, y eso no está empezado.

---

## 4. Decisiones y por qué (para no re-litigarlas)

1. **El editor va sobre el Core, no sobre el parser propio.** El Core entrega geometría ya deformada;
   el parser todavía no sabe componer deformadores. Usar el Core permitió tener editor en un día en
   vez de bloquearlo detrás del rig propio. El costo es la dependencia, y está asumido.
2. **Rasterizado propio en WebGL2**, no el del SDK. Alfa premultiplicado, `v' = 1 - v`, clamp de UV a
   medio téxel. Así la imagen es controlable píxel a píxel.
3. **Rig propio, no ingeniería inversa del Core.** `docs/09` lo dejó verificado: el framework oficial
   no contiene el algoritmo, solo llama al binario. Replicar `.moc3` byte a byte es un proyecto
   aparte; para animar basta un formato propio.
4. **`l2d-anim/1` guarda el vector completo de parámetros en cada clave**, no canales sueltos. Cuesta
   tamaño y elimina la pregunta «¿qué anima esta clave?»: cada clave es un estado reproducible.
   Especificado en `spec/01-formato-animacion.md`.
5. **CPU primero en Rust.** El rasterizador CPU es lento pero determinista, y es lo que permite
   comparar contra el oráculo del Core con margen de 0 píxeles.

---

## 5. Defectos encontrados, con causa

| Síntoma | Causa real | Arreglo |
|---|---|---|
| Modelo «despiezado», cada triángulo pinta otra zona del atlas | UV guardadas con origen arriba-izquierda; faltaba `v' = 1 - v` | invertir V en el muestreo |
| Halo/borde blanco en todo el contorno | atlas con alfa recto dibujado con mezcla premultiplicada | `o = vec4(c.rgb * c.a, c.a)` + clamp de UV a medio téxel |
| Cuatro brazos | mallas con máscara dibujadas sin recortar: se sumaban las dos variantes | esténcil por intersección |
| Bloque blanco tapando el torso | mallas fuente de máscara dibujadas como arte | excluir las fuentes de máscara del dibujo |
| Modelo deformado a cero | parámetros en 0 en vez de `defaultValues` antes de `update()` | copiar los valores por defecto |
| Cara que no aterriza | keyform elegido por heurística (primero/centro/mediana) | calibrado contra el oráculo: **el keyform de reposo es el 0** |

---

## 6. Discrepancias abiertas (números que no cuadran, sin maquillar)

1. **24 vs 72 parámetros.** `docs/08` declara 24 parámetros «del `.moc3`»; `l2d params` sobre Hiyori
   devuelve **72** y el editor construye uno por cada parámetro que declara el Core. Los 24 de
   `docs/08` son los parámetros que usan las mallas de arte; el archivo declara el resto (opacidades
   de parte, etc.). El documento 08 debe leerse con esa corrección.
2. **`art_mesh.keyform_begin_indices`.** Apunta a una tabla de 1 242 entradas, y en el render propio
   ~66 mallas leen bloques en escala de píxeles en vez de unidades: señal de índice local usado donde
   va el global (o al revés). Con `L2D_DEBUG=1` se ven las dos escalas conviviendo.
3. **`keyform_position.xys` es un pool compartido** entre mallas de arte y rejillas warp. Emparejar
   por coincidencia exacta contra el oráculo no funciona (error mínimo 20–100 px en 9 mallas
   probadas) porque el oráculo devuelve geometría **ya deformada**. Sigue sin documentación pública.

Ninguna de las tres bloquea el editor actual; las tres bloquean el visor independiente.

---

## 7. Cómo retomar (comandos exactos, en este orden)

```bash
cd ~/Proyectos/l2d-viewer

# 0. el repo ya existe: trabajar en dev (main es la rama estable)
git switch dev && git pull

# 1. verificar que sigue verde
cargo test
cargo run -p l2d -- params models/samples/Samples/Resources/Hiyori/Hiyori.moc3 | head

# 2. abrir el editor
tools/editor.sh            # Hiyori
tools/editor.sh Wanko      # el modelo pequeño, para probar rápido

# 3. comparar parser propio contra la verdad del Core
cargo run -p l2d -- oracle models/samples/hiyori.oracle.json \
  models/samples/Samples/Resources/Hiyori/texture_00.png out/hiyori.png 0.5
```

Siguiente trabajo, ya sin ambigüedad: **guardar el modelo editado** (formato propio `.l2d`, porque
escribir `.moc3` se pospuso en `docs/09`) y detrás **el evaluador propio en Rust** (`crates/pose`:
elegir keyform por valor de parámetro, luego warp y rotation). Los hitos H1–H5 de `docs/08` siguen
vigentes con la corrección de `docs/09`. Cargar animaciones se cerró el 16 nov (`docs/11`).

---

## 8. Criterio de aceptación (qué tendría que pasar para llamarlo terminado)

- **Visor independiente:** render de Hiyori sin Core, con error < 2 px por vértice contra el JSON del
  oráculo. Hoy no.
- **Editor completo:** cargar lo exportado, editar y borrar claves, guardar el modelo y reabrirlo con
  solo los cambios aplicados. Hoy: 3 de 4 (falta guardar; `docs/11`).
- **Animación:** `l2d-anim/1` de ida y vuelta sin pérdida. Hoy: **sí**, verificado en navegador el
  16 de noviembre.

---

## 9. Deudas y riesgos

- ~~**No es un repositorio git.**~~ **Resuelto el 16 nov** (`docs/11`): público en
  https://github.com/Evergaster/l2d-viewer, `main` estable y `dev` para desarrollo.
- **Los modelos de muestra no se versionan** (son de Live2D y su licencia no lo permite). Un clon
  nuevo no puede correr ni las pruebas de render ni el editor sin volver a bajarlos.
- **Límite legal**: el EULA del editor (§5.1.1) prohíbe portarlo o modificarlo; por eso este código
  es propio y el visor usa el Core **como binario**, no su código. Un producto publicable no puede
  llevar «Live2D» en el nombre (marca registrada). Ver `docs/01` y `docs/05`.
- **El editor depende de un CDN** (`cubism.live2d.com`) para el Core: sin red, no abre.

---

*Este cierre no dice «el visor está listo». Dice: el editor existe, sus números están medidos, sus
tres bloqueadores tienen nombre, y el siguiente paso era uno solo — cargar lo que ya se exporta
(hecho el 16 nov, `docs/11`).*
