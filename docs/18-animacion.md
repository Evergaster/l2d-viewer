# 18 — Abrir archivos y reproducir `l2d-anim/1` en el visor

Cierra los dos primeros pendientes de `docs/17` §4: el visor ya abre modelos y animaciones desde la
ventana, y reproduce la animación con el motor propio. Sigue sin cargar el Core.

---

## 1. Qué se agregó

| Pieza | Dónde | Qué hace |
|---|---|---|
| Lector de animación | `crates/view/src/anim.rs` | valida `l2d-anim/1`, ata los nombres a los índices del modelo e interpola |
| Selector de modelo | `crates/ui` → botón «Abrir modelo…» | abre `zenity` y carga el `.moc3` elegido |
| Selector de animación | botón «Abrir animación…» | filtra `*.json` y la valida contra el modelo abierto |
| Reproducción | `Msg::Cuadro` + `iced::time::every` | avanza con el **delta real** del reloj, en bucle, y redibuja |
| Banco de dibujo | `crates/view/examples/banco.rs` | ms por pose a cada ancho; es con lo que se eligió la cadencia |
| Diagnóstico | `L2D_UI_LOG=1` | cada 24 cuadros escribe el ms medio de dibujo en stderr |

```bash
tools/visor.sh                                   # Hiyori, sin animación
./target/release/l2d-ui <modelo.moc3> <anim.json> # abre y reproduce al arrancar
L2D_UI_LOG=1 tools/visor.sh                       # con el banco de cuadros en stderr
cargo run --release -p view --example banco -- <modelo.moc3>
```

---

## 2. Decisiones

1. **El selector es `zenity`, no `rfd`.** `rfd` en Linux arrastra `ashpd`/GTK y un árbol de
   dependencias nuevo para un diálogo; `zenity` ya está instalado en este escritorio (4.0.5) y se
   lanza desde una tarea de `iced`, así que no bloquea la interfaz. Si falta, el visor lo dice en el
   pie en vez de morir.
2. **`Animacion` se ata al modelo al cargar, no en cada cuadro.** Los nombres se traducen a índices
   una vez; evaluar sólo recorre las dos claves que rodean a `t`. Buscar cadenas 15 veces por
   segundo entre 70 parámetros habría sido trabajo regalado.
3. **Un parámetro que el modelo no tiene es error; uno del modelo que la animación no anima queda
   como esté.** Lo primero caza animaciones de otro modelo; lo segundo permite animar tres
   parámetros para probar sin exportar los 70. El editor exporta la lista completa, así que un
   archivo suyo cumple lo estricto.
4. **La animación manda mientras reproduce.** Mover un deslizador es posible, pero el siguiente
   cuadro lo pisa; al pausar, la pose queda congelada y se puede editar desde ahí. Es el
   comportamiento que se espera de un reproductor y no esconde estado.
5. **Dos anchos de dibujo.** 420 px mientras reproduce, 560 en pausa. Ver §4.

---

## 3. Reglas del formato, comprobadas

`spec/01-formato-animacion.md` ya las fijaba; ahora hay una prueba por regla en
`crates/view/src/anim.rs` (12 pruebas, más 2 de integración con el modelo real):

| Regla | Prueba |
|---|---|
| `formato` debe ser `l2d-anim/1` | `rechaza_formato_ajeno` |
| parámetro desconocido ⇒ archivo rechazado | `rechaza_parametro_que_el_modelo_no_tiene` |
| parámetro repetido ⇒ rechazado | `rechaza_parametro_repetido` |
| `t` creciente y sin repetir | `rechaza_claves_desordenadas_y_t_repetido` |
| `valores.len() == parametros.len()` | `rechaza_longitud_de_valores_que_no_cuadra` |
| sin claves o `duracion` ≤ 0 ⇒ rechazado | `rechaza_sin_claves_y_duracion_cero` |
| fuera de rango se mantiene la clave extrema | `fuera_de_rango_mantiene_la_clave_extrema` |
| interpolación lineal entre claves | `interpola_lineal_entre_dos_claves` |
| bucle sobre `duracion`, en ambos sentidos | `envolver_hace_bucle_en_ambos_sentidos` |
| un archivo rechazado no toca la animación abierta | en `actualizar`, `Msg::AnimacionCargada` |

**Integración con el modelo real** (`crates/view/tests/anim_real.rs`, se salta si no están los
modelos de muestra, porque no se versionan): carga Hiyori, aplica la animación de tres claves y
comprueba que la pose de `t=0` y la de `t=3` **no** son los mismos píxeles (hash FNV-1a 64 de la
imagen), que `ParamAngleX` toma el valor de la clave y no el de reposo, y que `envolver(4.2)=1.2`.
Deja `out/anim-t{0,15,30}.png` para mirar las tres poses y usa la animación de ejemplo
versionada (`spec/anim-ejemplo.json`, 3 claves sobre 5 parámetros reales de Hiyori).

---

## 4. Rendimiento: lo que se midió y lo que se decidió

`examples/banco.rs` sobre Hiyori (134 mallas, 24 partes, lienzo 2976×4175), en `release`:

| ancho | ms/pose | cuadros/s |
|---|---|---|
| 320 | 8,9 | 112 |
| 420 | 14,6 | 69 |
| 480 | 18,6 | 54 |
| 560 | 25,3 | 40 |
| 700 | 38,2 | 26 |
| 1400 | 139,0 | 7 |

Dentro de la ventana el mismo dibujo cuesta más (~29 ms a 420 px a 24 cuadros/s, ~21 ms a 15) porque
la caché se enfría entre cuadros. Con 24 cuadros/s y 560 px el visor se comía **85 % de un núcleo**;
con 15 cuadros/s y 420 px mide **39 %**. Eso es lo que quedó.

Lo que **no** mejoró: reutilizar un solo canvas de máscara en vez de crear uno por malla. El cambio
se quedó (menos memoria, misma imagen — los conteos de píxeles coinciden exactamente), pero el
tiempo no se movió: el costo está en el sombreado de triángulos, no en las reservas.

El cuello es el rasterizador: recorre el bbox de cada triángulo y por píxel hace 3 divisiones, un
muestreo bilineal (4 lecturas) y una composición en `f32`. Con 3 360 triángulos se sombrean bastantes
más píxeles que los 25 000 visibles. Las salidas serían SIMD, paralelizar por triángulo (con cuidado:
los triángulos de una malla comparten bordes) o una textura preconvertida. Ninguna se hizo: el visor
ya sostiene 15 cuadros/s sin comerse el equipo y el objetivo era tener el motor, no el rendimiento.

---

## 5. Los restos blancos del borde **no son un defecto del motor**

`docs/17` §3.1 dejó dos sospechosos para el filo claro en pelo y dedos: falta de clamp de UV a medio
téxel y empates de `draw_order`.

- El **clamp a medio téxel se añadió** (en la pasada de color y en la de máscara). Es lo correcto y
  coincide con lo que hace el editor en el shader, pero no cambia el síntoma.
- **No es nuestro.** Perfilado el borde contra el render del **Core oficial** (`out/hiyori-oracle.png`,
  guardado el 13 nov), el contorno tiene el mismo beige casi blanco: el oráculo da `(242,227,207)`
  donde el motor propio da `(242,235,214)`, y `(253,233,211)` en el mismo tramo del pelo. El atlas de
  Hiyori trae ese borde claro pintado a propósito (`matte`/bleed) para que al recortar contra fondos
  claros no aparezca negro.
- Los píxeles del borde son **opacos** (`a=255`), no semitransparentes: medido, 0 % de los 284
  píxeles semitransparentes del render son casi blancos. Un halo por sangrado de UV habría caído
  justo ahí.

Conclusión: quitarlo exigiría editar el atlas, no el motor. Queda como decisión del modelo, no
pendiente del código. El otro sospechoso (`draw_order` con empates) sigue sin tocar: no hay señal de
que sea el responsable y `draw_order_group` no está leído en `Body`.

---

## 6. Lo que queda de la interfaz

- **Editar vértices** desde la ventana: sigue viviendo en el editor del navegador (`docs/14`), que
  usa el Core.
- **Arrastrar el lienzo / zoom**: a 560 px el detalle fino no se juzga. Es lo siguiente útil si se
  va a revisar pintado.
- `draw_order_group` para desempatar órdenes de dibujo, con el modelo real como juez.
