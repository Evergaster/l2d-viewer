# Formato de animación `l2d-anim/1`

JSON legible. Una clave guarda **todos** los parámetros del modelo en un instante; la
interpolación entre claves es lineal por parámetro. Guardar el vector completo (en lugar de
canales sueltos) hace el archivo más grande pero elimina la pregunta "¿qué parámetros anima esta
clave?": cada clave es un estado reproducible.

```json
{
  "formato": "l2d-anim/1",
  "modelo": "Hiyori",
  "duracion": 5,
  "parametros": ["ParamAngleX", "ParamAngleY", "..."],
  "claves": [
    { "t": 0.0, "valores": [0, 0, -20, "..."] },
    { "t": 2.0, "valores": [0, 0, 20, "..."] }
  ]
}
```

Reglas:

- `parametros` fija el orden; `valores[i]` corresponde a `parametros[i]`. Si la lista no coincide
  con la del modelo, el archivo se rechaza al cargar.
- `claves` ordenadas por `t` ascendente; dos claves con el mismo `t` es error.
- Fuera del rango de claves se mantiene el valor de la clave extrema (sin extrapolar).
- `duracion` mayor que el último `t` deja una cola quieta; el reproductor hace bucle.
- Valores redondeados a 4 decimales: sobra precisión para una imagen de 8 bits.

Implementado en `editor/editor.html`: **Exportar .json** y **Cargar .json** (ida y vuelta
verificada, 140 cifras idénticas en la prueba de dos claves) más **Borrar clave**. La carga rechaza
el archivo con el motivo concreto si falla el formato, la lista de `parametros`, el orden de `t` o la
longitud de `valores`; un archivo rechazado no toca la animación abierta.
