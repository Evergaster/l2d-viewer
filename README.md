# l2d-viewer

Visor de modelos Live2D **en Rust, escrito desde cero**. No es un port, no es un parche y no
contiene ni una línea del editor de Live2D: es una implementación independiente del *formato*
y del *comportamiento*, no del código.

> El nombre del directorio es de trabajo. Un producto publicable **no** debe llevar «Live2D» en
> el nombre (marca registrada); ver `docs/05-licencias.md`.

## Estado

| Pieza | Estado |
|---|---|
| Lector del contenedor `.moc3` (cabecera, canvas, tabla de secciones) | ✅ probado contra 8 modelos reales |
| Lector de `.model3.json` | ✅ |
| Rasterizador CPU (triángulos, UV, blend modes, máscaras) → PNG | ✅ |
| Cuerpo del modelo: partes, mallas, parámetros, keyforms, máscaras | ✅ verificado contra `py-moc3` |
| Orden de dibujo por parte + visibilidad | ✅ |
| Selección de keyform por parámetro | ✅ calibrada contra el Core: el keyform de reposo es el 0 (`docs/07`) |
| **Evaluador propio (deformadores warp/rotation)** | ⬜ **bloqueador del visor independiente** |
| Editor: sliders, vértices, capas, máscaras, claves, timeline, export | ✅ prototipo funcional (sobre el Core) |
| Cargar lo exportado, editar/borrar claves, guardar el modelo | ⬜ |
| `draw_order_group`, ventana nativa | ⬜ |

> Estado real: lector de formato sólido, renderizador de geometría correcto y un **editor usable**
> que deforma con el Core oficial. Unirlos (rig propio en Rust) no está empezado.
> Cierre del tramo editor y siguiente paso: `docs/10-cierre-del-editor.md`.

```bash
cargo test                              # 12 pruebas + 1 doctest
cargo run -p l2d -- render modelo.moc3 textura.png out.png 0.3
cargo run -p l2d -- body modelo.moc3    # partes, mallas, parámetros
cargo run -p l2d -- params modelo.moc3  # parámetros reales con min/max/defecto
cargo run -p l2d -- inspect modelo.moc3
tools/editor.sh                         # editor (Hiyori); tools/editor.sh Wanko
```

## Documentos

1. `docs/01-protocolo-cuarentena.md` — **el más importante**: qué se puede mirar y qué no.
2. `docs/02-arquitectura.md` — crates, fronteras y por qué Rust.
3. `docs/03-formato-moc3.md` — el contenedor, verificado contra especificación pública.
4. `docs/04-hoja-de-ruta.md` — de visor a editor, con hitos binarios.
5. `docs/05-licencias.md` — qué licencia tiene *este* código y qué pasa con el SDK.
6. `docs/06-primer-render.md` — qué se dibuja hoy, qué falta y por qué.
7. `docs/07-cierre-de-fase.md` — cierre de la fase de investigación del formato.
8. `docs/08-editor-arquitectura.md` — arquitectura del editor e hitos H1–H5.
9. `docs/09-sdk-sin-algoritmo.md` — por qué el SDK no trae el algoritmo de deformación.
10. `docs/10-cierre-del-editor.md` — **el estado de hoy**: qué hay, qué no, cómo retomar.

## La regla

Este repositorio **no lee** `~/Proyectos/live2d-cubism-linux/dist/app/lib/Live2D_Cubism.jar`.
Ni descompilado, ni traducido, ni «inspirado en una función concreta del bytecode».
Ver `docs/01`.


## Estado (2026-11-13)

**Render visualmente correcto, verificado con dos modelos oficiales (Hiyori, Wanko).**

```bash
l2d oracle <dump.json> <texture_00.png> <salida.png> [zoom]
```

`oracle` renderiza a partir de la geometría que entrega el **Core oficial** (SDK de Live2D),
volcada a JSON por `models/samples/dump.html`. Sirve para dos cosas: tener un visor que se ve
bien hoy, y como verdad de referencia para validar el parser propio.

Hallazgos que hicieron posible el render correcto:

| Causa | Efecto |
|---|---|
| UV con V sin invertir | modelo "despiezado": cada triángulo pinta otra zona del atlas |
| Mallas fuente de máscara dibujadas como arte | bloque blanco tapando el torso |
| Parámetros sin `defaultValues` | deformación a cero |

Pendiente para que el visor sea **independiente** (sin Core): resolver keyform por malla y la
cadena de deformadores. Ahora hay contra qué validarlo: el JSON del oráculo.


## Editor: primer ladrillo

```bash
l2d params <modelo.moc3>
```

Enumera los parámetros reales del modelo (id, mínimo, máximo, valor por defecto) leyendo el
`.moc3` con el parser propio. Es la lista sobre la que se construye el animador: cada fila es un
control que el usuario va a mover.

Hiyori declara 24 parámetros: ángulos de cabeza, apertura y sonrisa de ojos, mirada, cejas,
boca, respiración, posiciones de brazos y cuerpo. Los valores por defecto son los que definen la
pose de reposo que ya se renderiza correctamente.


## Editor (prototipo funcional)

```bash
tools/editor.sh            # Hiyori por defecto
tools/editor.sh Wanko
```

Sirve `models/samples` por HTTP y abre `editor.html`: lienzo con el personaje y **panel de
sliders** para todos sus parámetros (70 en Hiyori). Al mover un slider, el modelo se deforma y
se redibuja en el acto.

Cómo está hecho: geometría y deformación por el **Core oficial** (SDK); rasterizado propio en
**WebGL2** con alfa premultiplicado y `v' = 1 - v`. Se excluyen del dibujo las mallas que son
fuente de máscara y el `HitArea` (área de impacto, no arte).

![prototipo](docs/editor-prototipo.png)

### Edición de vértices

Botón **Editar vértices**: aparecen los nodos de la malla; arrastrar uno lo mueve y la malla se
deforma en vivo. **Deshacer** revierte (pila de estados). Los desplazamientos se guardan por
`malla:vértice` y se aplican al subir los vértices a la GPU, de modo que la deformación del Core
y la edición del usuario conviven sin pisarse.

![edición de vértices](docs/editor-vertices.png)

### Sistema de capas y máscaras

Dos defectos corregidos, ambos del sistema de capas:

| Síntoma | Causa | Arreglo |
|---|---|---|
| Halo/borde blanco en todo el contorno | el atlas usa alfa recto y se dibujaba con mezcla premultiplicada sin premultiplicar | `o = vec4(c.rgb * c.a, c.a)` y clamp de UV a medio téxel |
| Cuatro brazos | las mallas con máscara se dibujaban sin recortar: las variantes de brazo se sumaban | esténcil: se escribe la forma de cada máscara y la malla se dibuja solo donde todas coinciden |

El esténcil se incrementa por cada máscara y la malla exige `EQUAL` al número de máscaras, que es
el recorte por intersección que usa Cubism.

### Panel de capas

Arriba del panel, una casilla por **parte** del modelo (24 en Hiyori). Ocultar una parte apaga
todas sus mallas. Por defecto se ocultan las variantes mutuamente excluyentes del nombre
`PartArmA` / `PartArmB`: el modelo trae dos juegos completos de brazo y solo uno debe estar activo.
Sin eso aparecen cuatro brazos.

![capas](docs/editor-capas.png)

La opacidad con la que se dibuja cada malla es **malla × parte × visibilidad de capa**.

### Claves y timeline

Panel **Animación**: se fija un tiempo, se mueven los sliders y se pulsa **Añadir clave**.
**Reproducir** interpola linealmente entre claves y hace bucle; **Exportar .json** descarga la
animación en el formato `l2d-anim/1` (`spec/01-formato-animacion.md`).

![timeline](docs/editor-timeline.png)

Verificado: dos claves (`AngleZ` −20 en 0 s, +20 en 2 s) dan 0 en 1 s y 10 en 1,5 s; la
reproducción avanza con el reloj real y repinta.

Lo que falta: **cargar** una animación exportada, editar/borrar claves y guardar el modelo. Ver
`docs/08-editor-arquitectura.md`.

## Ramas

`main` es la rama estable (lo que está probado y documentado); todo el desarrollo va en `dev`.
`main` solo se mueve cuando `dev` está estable, a mano.

## Licencia y alcance

Código bajo `MIT OR Apache-2.0` (ver `LICENSE-MIT` y `LICENSE-APACHE`); `docs/05-licencias.md`
explica el resto.

- Proyecto **independiente**, sin relación con Live2D Inc. «Live2D» es marca de su titular y aquí
  solo se nombra el formato que este código lee.
- No redistribuye binarios ni código de Live2D, ni modelos de muestra: `models/` está en
  `.gitignore` porque cada modelo tiene su propia licencia.
