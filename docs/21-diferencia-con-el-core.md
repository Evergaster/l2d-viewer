# 21 — Los 8,6 % que faltan: supermuestreo y qué NO es la diferencia

Después de cerrar el orden de dibujo (`docs/20`) quedaba un 8,6 % de píxeles que no coinciden con el
Core. Esta ronda añade supermuestreo y, sobre todo, **descarta** hipótesis con números.

---

## 1. Lo que se añadió: supermuestreo

`raster::Canvas::reducir(factor)` promedia bloques de `factor × factor` en **alfa premultiplicado**
(como hace un multisample real; promediar RGB con alfa recta saca un filo oscuro). `Canvas` no tenía
antialias de ninguna clase, así que esto es lo único que suaviza de verdad.

| API | Qué hace |
|---|---|
| `raster::Canvas::reducir(factor)` | reducción por promedio premultiplicado |
| `view::Vista::render_ssaa(…, factor)` | dibuja a `factor ×` y reduce |
| `l2d ver … --ssaa N` | N = 1 (defecto), 2, 3, 4 |
| `l2d ver … --ssaa 3 --zoom 6 --centro X,Y` | el caso de uso: mirar de cerca sin escalones |

Cuatro pruebas nuevas (`pruebas_reducir`): factor 1 no cambia nada, un cuarto de muestra deja alfa
0,25 con el color intacto, el borde no arrastra el color de lo transparente (el caso
"transparente pero blanco", que es el que produce halos), y el tamaño no se deforma con factores que
no dividen exacto.

**Y no se activó por defecto.** Medido, no vale la pena:

| ssaa | diferencia vs Core (sólo opacos) | píxeles Δ>60 | diferencia con bordes | tiempo a 1042 px |
|---|---|---|---|---|
| 1 | 19,41 | 8,61 % | 4,44 | 79 ms |
| 2 | 18,77 | 8,60 % | 4,35 | 352 ms |
| 3 | 17,19 | 7,98 % | 4,05 | 768 ms |

Baja un 11 % la diferencia y cuesta 4,5× más. Se queda como opción para exportar, con el defecto en
1 para no cambiar el comportamiento de nadie sin ganancia.

---

## 2. Lo que se descartó (y cómo)

La pregunta era de dónde salen esos píxeles. Estas son las hipótesis y su resultado:

| Hipótesis | Prueba | Resultado |
|---|---|---|
| Es todo antialias de silueta | comparar sólo píxeles interiores (erosión 5×5) vs opacos | **falso**: interior 18,68 y 8,26 % >60; borde 32,43 y 14,74 %. La diferencia existe también dentro de las piezas |
| Falta la opacidad de partes/deformadores | `moc3/examples/opacidades.rs` sobre las cuatro secciones | **falso**: mallas 0 valores fuera de 1, warp y rotación **todas 1,000**. No hay `part_keyform.opacities` en el formato (PartForm sólo tiene `depth`) |
| Blend modes por malla (multiplicativo/aditivo) | `flags` de los 134 drawables en el volcado del Core | **improbable**: los 134 tienen `flags = 4`, uniforme |
| Desfase de la comparación | desplazar nuestra imagen ±1 px y volver a medir | **parcial**: con (0,−1) la diferencia baja de 19,38 a 18,01. Nuestro alto se redondea a 1462 y el del oráculo a 1461, así que la comparación arrastra medio píxel |
| Detalle fino de textura | desenfocar ambas 3×3 antes de comparar | **parcial**: 19,38 → 14,70; casi un cuarto de la diferencia es alta frecuencia (líneas del dibujo, brillos) |

Lo que queda vivo, sin medir todavía:

1. **Filtrado de textura.** El atlas es 2048 y el render 1042: estamos reduciendo a la mitad sin
   mipmaps. El Core muestrea con pirámide; nosotros con bilineal a pelo. Explicaría la parte de alta
   frecuencia que sobrevive al desenfoque.
2. **`multiply_color` / `screen_color` (V4.2+).** Están en el formato como conteos
   (`keyform_multiply_colors`, `keyform_screen_colors`) y **no se leen**. Si alguna malla los usa, el
   color cambia en toda su superficie. Es la siguiente cosa barata de comprobar: leer las secciones y
   ver si alguna no es blanco.

---

## 3. Regla de método que sale de aquí

Tres veces en esta ronda la intuición fue peor que el instrumento: el antialias "obvio", la opacidad
"que seguro faltaba" y la descripción de la imagen ("bordes escalonados") fueron todas falsas. La
única forma de avanzar fue medir contra una referencia y **descartar** con el número en la mano.

Corolario práctico: cada hipótesis de este documento tiene su comando en el repositorio
(`examples/opacidades.rs`, `examples/grupos.rs`, `examples/comparar.rs`, `--ssaa`, `l2d oracle`). La
siguiente persona no debería volver a empezar de cero.
