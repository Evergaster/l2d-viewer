# 05 — Licencias

## Este código

`MIT OR Apache-2.0` (dual, como la mayoría del ecosistema Rust). Es tuyo, se puede publicar.

Pero la licencia del código no resuelve el resto:

| Pieza | Régimen | Qué implica |
|---|---|---|
| Código de este repo | MIT/Apache-2.0 | libre de publicar y modificar |
| Formato `.moc3` | no es propiedad intelectual protegible (hecho/idea) | implementarlo es interoperabilidad |
| Especificación de OpenL2D | Free Development Public License 1.0-US | se citan **hechos** del formato, no se copia su código |
| Modelos y texturas | licencia de cada modelo | **nunca** al repo; `.gitignore` ya cubre `models/` |
| Cubism Core / SDK | propietario de Live2D | solo si se enlaza; ahí aplican los términos del SDK |
| Marca «Live2D» | registrada | **no** se usa en el nombre del producto |

## Sobre el nombre

`l2d-viewer` es nombre de carpeta. Un producto que se distribuya **no** debe llamarse
«Live2D algo» ni insinuar que es oficial: eso es confusión de marca y le da a Live2D un motivo
para escribirte que no tiene nada que ver con el copyright. Un nombre neutro y una nota de
"no afiliado" es lo correcto.

## Si algún día se distribuye

1. Si el proyecto sigue siendo 100 % código propio (sin `libLive2DCubismCore`), no hay licencia de
   Live2D que cumplir por el *software*; sí por los modelos que el usuario cargue.
2. Si se enlaza el Core oficial, aplican los términos del SDK; para uso comercial, además, la
   *SDK Release License*.
3. Antes de publicar en un repo público, releer `docs/01-protocolo-cuarentena.md` y comprobar que
   el historial de git demuestra que el código nació de la spec.

## Lo que este proyecto NO hace

- No redistribuye binarios de Live2D.
- No toca ni documenta el sistema de licencias del editor.
- No incluye modelos de muestra de terceros.
