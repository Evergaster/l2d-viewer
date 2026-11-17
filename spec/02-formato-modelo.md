# Formato de modelo editado `l2d-model/1`

JSON legible que guarda **solo lo que el usuario cambió** sobre un `.moc3`: los parámetros que no
están en reposo, las partes ocultas o mostradas, y los desplazamientos de vértices. El `.moc3` sigue
siendo la base y **no se reescribe** (escribir `.moc3` está pospuesto, `docs/09`); este archivo es la
capa de cambios encima.

```json
{
  "formato": "l2d-model/1",
  "modelo": "Hiyori",
  "origen": { "archivo": "Hiyori.moc3", "bytes": 307123, "huella": "9f2c1d4a0b7e6f83" },
  "parametros": { "ParamAngleX": 12.5, "ParamEyeLOpen": 0.4 },
  "partes": { "PartArmB": false },
  "vertices": [
    { "malla": "ArtMesh23", "vertice": 12, "dx": 0.25, "dy": -0.5 }
  ]
}
```

## Reglas

- `formato` debe ser exactamente `l2d-model/1`; otro valor rechaza el archivo.
- `origen` identifica el `.moc3` sobre el que se hicieron los cambios:
  - `bytes` = tamaño del archivo en bytes.
  - `huella` = **FNV-1a de 64 bits** sobre todos los bytes del `.moc3`, en hexadecimal minúsculas de
    16 cifras (16 iteraciones por byte: `h ^= b; h *= 0x100000001b3`, offset `0xcbf29ce484222325`).
    Se eligió FNV-1a porque son diez líneas en Rust y en JavaScript, sin dependencias, y basta para
    distinguir dos modelos; no es una firma criptográfica.
  - Al cargar, si `bytes` o `huella` no coinciden con el modelo abierto, el archivo se rechaza con
    los dos valores en el mensaje: los índices de vértice no significan lo mismo en otro modelo.
- `parametros` guarda **solo** los parámetros que difieren del valor por defecto del modelo
  (tolerancia `1e-4`, redondeo a 6 decimales). Un parámetro que no existe en el modelo rechaza el
  archivo.
- `partes` guarda **solo** las partes cuya visibilidad difiere de la de arranque del modelo
  (tolerancia al `PartArmA`/`PartArmB` que el editor oculta por defecto). `false` = oculta.
- `vertices` es una lista plana de desplazamientos independientes, no una malla aparte:
  - `malla` = id del drawable (`ArtMesh23`), no su índice; el índice cambia entre versiones del
    archivo, el id no.
  - `vertice` = índice **0-based** del vértice dentro de esa malla. Fuera de rango rechaza el archivo.
  - `dx`, `dy` en **unidades del modelo** (las mismas de `vertexPositions` del Core; multiplicar por
    `ppu` da píxeles), redondeados a 6 decimales. Se omiten los desplazamientos nulos.
- Las claves ausentes (`parametros`, `partes`, `vertices`) equivalen a vacío. El archivo describe el
  estado editado **completo**, no un parche incremental: al cargarlo el modelo vuelve a su estado base
  (parámetros por defecto, visibilidad de arranque, sin desplazamientos) y encima se aplican los
  cambios. Por eso cargar el mismo archivo dos veces deja lo mismo, y un archivo con todo en reposo
  devuelve el modelo intacto.
- La animación **no** va aquí: es `l2d-anim/1` (`spec/01-formato-animacion.md`). Un proyecto son dos
  archivos, y cada uno se puede reutilizar por separado.

## Semántica del desplazamiento (límite conocido, no un defecto pendiente)

El desplazamiento se suma a la posición que entrega el **Core** con los parámetros guardados. No
vive en el espacio local del deformador, así que si después se mueve un parámetro el vértice editado
viaja con la deformación en vez de quedarse fijo respecto a su malla. Quitar esa limitación exige el
rig propio (deformadores warp/rotation, `docs/12`), no un cambio de formato: los campos `malla` y
`vertice` son los mismos que necesitará el rig.

## Ida y vuelta

`editor/editor.html` → **Guardar .l2d** escribe el archivo y **Abrir .l2d** lo vuelve a aplicar
sobre el modelo recién cargado. Criterio de aceptación: mismo estado de parámetros, mismas partes
ocultas, mismos desplazamientos y **los mismos píxeles** (huella del framebuffer idéntica) antes de
guardar y después de reabrir.

`l2d edit <archivo.l2d> <modelo.moc3>` valida el archivo contra el `.moc3` desde Rust (huella,
mallas, rango de vértices, parámetros, partes) sin abrir navegador.
