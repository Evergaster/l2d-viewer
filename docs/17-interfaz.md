# 17 — El visor con interfaz (`l2d-ui`)

Primera interfaz gráfica del proyecto. Muestra el modelo dibujado por el **motor propio** y deja
mover sus parámetros. No carga el Core de Live2D en ningún momento.

---

## 1. Qué hay

| Pieza | Dónde | Qué hace |
|---|---|---|
| Dibujo de una pose | `crates/view` | junta `pose` (geometría exacta, `docs/16`) con `raster` (CPU): mallas, texturas, máscaras y orden de dibujo |
| Comando de línea | `l2d ver <modelo.moc3> <salida.png> [--ancho N] [--param Nombre=valor]` | el mismo dibujo, a PNG, sin ventana |
| Capas | `view::Vista::partes` + `ocultas_por_defecto()` | una casilla por parte; respeta `part_visible` del archivo y oculta la variante `…B` de cada par `…A`/`…B` |
| Visor | `crates/ui` → `l2d-ui` | ventana con **capas** y un deslizador por parámetro, «Volver a reposo» y «Guardar PNG» |
| Lanzador | `tools/visor.sh [modelo.moc3]` | compila si hace falta y abre el visor con el EGL de Mesa exportado |

```bash
tools/visor.sh                                   # Hiyori
tools/visor.sh ~/Proyectos/modelos/Otra.moc3     # cualquier modelo
cargo run -p l2d -- ver …/Hiyori.moc3 out.png --ancho 700 --param ParamAngleZ=10
```

El PNG es la misma imagen que enseña la ventana (mismo código), así que sirve para comprobar el
motor sin abrir interfaz: se generó uno a 700 px y se revisó a ojo —personaje completo, de frente,
sin piezas sueltas— antes de dar el visor por bueno.

---

## 2. Decisiones (y por qué)

1. **Se dibuja en CPU, no con shaders.** Es determinista, coincide exactamente con lo que miden
   las pruebas (`134/134` contra el Core) y no depende del EGL del equipo, que aquí es
   quisquilloso por la NVIDIA. A 560 px de ancho tarda ~0,1 s en `debug`; en `release`, bastante
   menos.
2. **Una ventana normal con `iced` 0.14**, el mismo toolkit que el shell del usuario: cero
   sorpresas de integración y un tema claro/oscuro de regalo.
3. **Un deslizador por parámetro, con su valor a la vista.** Los ids salen sin el prefijo `Param`
   (`AngleZ`, `EyeLOpen`…), que es como se leen. 70 deslizadores en una lista con scroll.
4. **Tres botones y nada más**: reposo, guardar PNG, cambiar tema. Cada uno hace lo que dice.

---

## 3. Lo que apareció al dibujar (defectos reales, corregidos)

Tres cosas que el motor no tenía resueltas porque nadie había mirado una imagen:

| Síntoma | Causa | Arreglo |
|---|---|---|
| Personaje boca abajo | la `y` del Core está invertida respecto del lienzo del archivo | `y' = alto − y` al pasar a píxeles |
| Piezas despiezadas, cada triángulo con otro trozo de textura | doble inversión de la `v` (el atlas se muestrea con `v` tal cual) | UV sin invertir, en pareja con el volteo de `y` |
| El modelo salía de espaldas | se pintaba con el `draw_order` **descendente** | ascendente: primero el fondo, después lo de delante |

También apareció un **defecto del lector `model3`**: rechaza `Hiyori.model3.json` con
`invalid type: map, expected a string at line 88` (campo `Motions`). El visor lee sólo
`FileReferences.Textures` con `serde_json` y lo deja anotado como pendiente del crate.

---

---

## 3.1 Lo que todavía se ve sucio (medido a ojo, no arreglado)

Con las máscaras y las variantes `PartArmA/B` corregidas, el sprite sale limpio de bulto, pero al
acercarse quedan **restos blancos en los bordes del pelo y de los dedos**. Dos sospechosos, ninguno
confirmado:

1. **Sin clamp de UV a medio téxel.** El editor del navegador lo hace y evita que el bilineal
   arrastre color de la región vecina del atlas; el rasterizador CPU no lo hace.
2. **Empates de `draw_order`.** Varias mallas comparten clave de orden y se pintan en orden
   arbitrario; Cubism desempata con `draw_order_group`, que aquí no se usa.

Ninguno de los dos afecta a la geometría (que sigue medida en 134/134): son defectos de pintado.

## 4. Lo que falta de la interfaz

- ~~Abrir modelos desde la ventana~~: **hecho**, selector del sistema (`zenity`). Ver `docs/18`.
- ~~Reproducir `l2d-anim/1`~~: **hecho**, en bucle y con delta real de reloj. Ver `docs/18`.
- **De §3.1**: el clamp de UV a medio téxel ya está; el filo claro del borde **no** es del motor —
  el render del Core oficial tiene el mismo beige (medido en `docs/18` §5).
- **Editar vértices** desde esta ventana: hoy eso vive en el editor del navegador (`docs/14`),
  que usa el Core. Traerlo aquí exige el rig propio completo, no sólo la pose.
