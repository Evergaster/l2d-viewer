# 19 — Encuadre: zoom y centro de la cámara

A 560 px el detalle fino no se juzga (pestañas, dedos, borde del pelo). El visor ya puede
acercarse: el encuadre es una pieza de `view`, así que sirve igual en la ventana, en `l2d ver` y en
el PNG guardado.

---

## 1. Qué hay

| Pieza | Dónde | Qué hace |
|---|---|---|
| `view::Encuadre` | `crates/view/src/lib.rs` | `zoom` + `centro`; `entero(vista)`, `con_zoom(z)`, `centrado_en(vista, x, y)` |
| `Vista::render_encuadre` | idem | el dibujo con la cámara puesta; `render` es el caso 1:1 |
| CLI | `l2d ver … --zoom N --centro X,Y` | PNG con la cámara puesta |
| Ventana | rueda · `+` `-` · flechas · `0` · botones `+` `−` `Todo` | la misma cámara, en vivo |
| Arranque | `l2d-ui <modelo> --zoom 4 --centro 1488,700` | abre el visor ya en la cara |

```bash
l2d ver modelo.moc3 /tmp/cara.png --ancho 560 --zoom 4 --centro 1488,700
l2d-ui modelo.moc3 --zoom 5 --centro 900,2300          # la mano, al abrir
L2D_UI_LOG=1 l2d-ui modelo.moc3 --zoom 4 --centro 1488,700   # imprime el encuadre aplicado
```

`--centro` (y el resto de la interfaz) usan **píxeles de imagen**: `x` desde la izquierda, `y` desde
arriba, como se lee en un PNG de 2976×4175.

---

## 2. Decisiones

1. **El encuadre se aplica al pasar de unidades a píxeles**, no a la geometría. `to_screen` pasa de
   `x·k` a `x·k + mitad − centro·k`. Eso deja intactos el motor, las máscaras y el orden de dibujo:
   si la geometría está bien a 1×, lo está a 8×. Y `Encuadre::entero` da **exactamente** el dibujo
   anterior, con prueba de igualdad de píxeles.
2. **El marco no cambia con el zoom.** El alto de salida sigue siendo el del lienzo completo, así
   que acercarse recorta, no deforma. Se guarda el PNG con la misma proporción.
3. **Nada de arrastrar con el ratón.** La rueda acerca, las flechas mueven, `0` vuelve. Arrastrar
   exige coordinar pulsación, movimiento y captura de puntero con los widgets del panel, y para QA
   visual (mirar la cara, mirar la mano) el teclado sobra.
4. **Topes**: zoom 0.5–12 en la ventana (16 en la API), centro recortado al lienzo, `NaN` cae a 1×.
   Más de 16× sólo amplía téxeles.

---

## 3. La trampa que costó una vuelta

El eje vertical del modelo **apunta hacia arriba** (es el eje en el que trabaja `to_screen`), pero
una persona lee coordenadas de imagen de arriba abajo. La primera versión tomaba `--centro 1488,700`
como `y = 700` en el eje del modelo, y el resultado fue un primer plano de **las piernas** — el
número era correcto, el eje no.

Por eso `centrado_en` recibe coordenadas de imagen y las invierte (`centro.y = alto − y`), mientras
que el campo `Encuadre::centro` queda documentado como eje del modelo. Comprobado por log:
`--centro 1488,700` imprime `centro (1488,3475)` y la imagen sale en la cara.

---

## 4. Verificación

Cinco pruebas en `crates/view/tests/encuadre.rs` (se saltan si no están los modelos de muestra):

| Prueba | Qué asegura |
|---|---|
| `entero_es_identico_a_render` | 1:1 es **el mismo array de píxeles**, no "parecido" |
| `el_zoom_conserva_el_centro` | el píxel central no se mueve a 1,5×/2×/3× (±3 por el bilineal) |
| `el_zoom_agranda_el_personaje` | a 2× hay más píxeles del modelo que a 1× |
| `el_centro_mueve_la_camara` | cambiar el centro cambia la imagen y sigue dibujando algo |
| `entradas_absurdas_se_recortan` | `zoom = NaN`, zoom negativo y centro fuera del lienzo no revientan |

Y a ojo, con `vision_analyze` sobre los PNG generados:

- `--zoom 4 --centro 1488,700`: primer plano del rostro con **iris con brillo, pestañas y boca
  abierta** distinguibles, sin huecos ni piezas desalineadas.
- `--zoom 5 --centro 900,2300`: mano con los dedos completos y contorno limpio (el pulgar oculto es
  del propio dibujo, no del motor).

**No verificado**: la rueda y las flechas dentro de la ventana. Se ejercitaron el estado, el render
y el arranque con `--zoom/--centro` (por log y por PNG), pero mover el ratón sobre la ventana no se
automatizó.

---

## 5. Coste

El zoom **no** añade triángulos: añade área. A 4× y 560 px de ancho cada malla cubre mucho más
lienzo, así que el dibujo pasó de 24 ms (1×) a 118 ms en `l2d ver`. En la ventana eso significa que
acercarse mucho y reproducir a la vez va a arrastrarse: el orden natural es pausar, acercarse,
mirar, y volver con `Todo`/`0`.

Si algún día molesta, la salida es recortar el trabajo a las mallas cuyo bbox cae fuera del marco
(ahora se descartan solas al quedar fuera del canvas, pero se sigue evaluando su geometría).
