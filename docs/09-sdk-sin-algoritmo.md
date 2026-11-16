# 09 — Hallazgo: el SDK no contiene el algoritmo de deformación

Verificado leyendo el código, no supuesto.

## Qué dice el framework oficial

`Live2D/CubismWebFramework` (develop), `src/model/cubismmodel.ts`:

```ts
public update(): void {
  // Update model
  this._model.update();              // <- llamada al Core (binario)
  this._model.drawables.resetDynamicFlags();
}
```

```ts
public getDrawableVertexPositions(drawableIndex: number): Float32Array {
  const verticesArray: Float32Array[] = this._model.drawables.vertexPositions;
  return verticesArray[drawableIndex];   // <- vienen ya deformadas del Core
}
```

Es decir: el framework **no evalúa keyforms ni deformadores**. Solo pasa parámetros al Core
(`csmUpdateModel`) y lee los vértices que el Core calcula. Lo mismo hace `CubismNativeFramework`
en C++: es una capa de conveniencia sobre el mismo binario.

## Consecuencia

No existe implementación abierta del algoritmo de deformación en la organización de Live2D. Está
en el Core, que se distribuye compilado. Por tanto:

- Descifrar el empaquetado interno de `keyform_position.xys` con fidelidad byte a byte exige
  ingeniería inversa del binario del Core. Es viable, pero es un proyecto por sí mismo.
- **No es necesario para tener un editor.** Un editor define *su propio* modelo de rig: usted
  decide cómo se guardan las claves y cómo se interpolan. Replicar el formato interno de Cubism
  solo hace falta para leer/escribir `.moc3` de terceros (hito H5), no para animar.

## Consecuencia para el plan (corrige `docs/08`)

| Hito | Antes | Ahora |
|---|---|---|
| H1 pose por parámetros | dependía del pool de keyforms | se hace con **rig propio**, sin Core |
| H2 edición de vértices | igual | igual |
| H3 claves de parámetro | igual | formato propio, documentado |
| H4 timeline | igual | igual |
| H5 guardar `.moc3` | igual | se pospone; primero formato propio (`.l2d`) |

El visor de modelos existentes sigue funcionando con `l2d oracle` (Core para geometría, nuestro
rasterizador para dibujar). El editor no necesita el Core: trabaja sobre el rig propio.

## Puente inmediato (funciona hoy)

Un dump del Core acepta cualquier valor de parámetro antes de `update()`. Eso permite ya
el ciclo "muevo un slider → veo el cambio" sin escribir el evaluador:

```js
const P = model.parameters;
for (let i = 0; i < P.count; i++) P.values[i] = P.defaultValues[i];
P.values[P.ids.indexOf('ParamAngleX')] = 15;
model.update();
```

Es la maqueta con la que se valida la interfaz (panel de parámetros, timeline) mientras el rig
propio se implementa en Rust.
