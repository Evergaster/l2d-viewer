# 16 — El motor de deformación, cerrado (17 nov 2026)

Cierra el bloqueo que arrastraban `docs/12`, `docs/13` (17 hipótesis descartadas) y `docs/15`.
El motor propio reproduce el Core **en todas las mallas**, en la pose de reposo y en poses
movidas.

---

## 1. De dónde salió la respuesta

No del SDK. De dos fuentes independientes y verificables, como pidió el usuario:

| Fuente | Qué aportó |
|---|---|
| `AyagamiDev/docs · model/moc3_file_format.md` (especificación no oficial, reversa independiente) | § «Deformer chaining»: cada deformador deforma a sus **hijos inmediatos**, y el caso *rotación hija de un warp* es especial (escala heredada, traslación deformada, ángulo sondeado) |
| [`mocari`](https://docs.rs/mocari) 0.4.1 (Rust, MIT) | el algoritmo completo: `moc3/compose.rs` (composición y sondeo), `core/deformers.rs` (bilineal + extrapolación), `moc3/keyform_bindings.rs` (selección **con pesos**) |

Las dos piezas que faltaban:

1. **Composición.** No se recorre la cadena aplicando deformadores a los vértices. Se compone
   cada deformador con su padre, de la raíz a las hojas:
   - la rejilla de un warp se deforma con el padre compuesto;
   - una rotación hereda la **escala acumulada** del padre, su **traslación se deforma** por el
     padre, y su **ángulo** se corrige sondeando `(0, −0.1)` (`−10` si el padre es rotación) y
     midiendo cuánto gira el vector;
   - la malla se transforma **una vez**, por su padre directo.
2. **Selección con pesos.** El Core **interpola** entre los dos keyforms vecinos de cada eje
   (`t = (v−k_i)/(k_{i+1}−k_i)`, pesos `1−t`/`t`, multiplicados entre ejes). No elige «el más
   cercano». En la última clave el activo es el de la **derecha** — la «excepción de la banda 7»
   de `docs/13` §5.7 deja de ser una excepción.

---

## 2. Números (Rust y Python, mismo resultado)

`l2d pose <moc3> --oraculo <dump.json>`, métrica de `docs/13` (mayor diferencia por coordenada):

| Modelo / pose | Exactas (< 1e-3) | Mediana | Máximo |
|---|---|---|---|
| Hiyori, reposo | **134/134** | 4,9·10⁻⁶ | 0,00020 |
| Wanko, reposo | **34/34** | 4,6·10⁻⁶ | 0,000005 |
| Hiyori, `ParamAngleZ=10` | **134/134** | 4,9·10⁻⁶ | 0,00065 |
| Hiyori, `ParamAngleZ=−10` | **134/134** | 4,9·10⁻⁶ | 0,00040 |
| Hiyori, `ParamEyeBallX=1` | **134/134** | 4,9·10⁻⁶ | 0,00020 |

Todas por debajo de 2 px (`6,7·10⁻⁴`… salvo el máximo de 6,5·10⁻⁴ de `AngleZ=10`, que está justo
en el límite: es el techo de precisión de la extrapolación afín, ver §4).

Antes de esto: **12/134**. El delta entre poses, que llevaba fallando desde `docs/13` §5.10
(«el modelo se mueve la mitad», 0/93 mallas bajo el 10 % de error), ahora cierra exacto.

---

## 3. Qué era el «rectángulo de dominio» de `docs/13`

Un artefacto. La calibración de §5.5–§5.16 encontró rectángulos por warp (`Warp1`:
0,5690 × 0,7982) con los que la **pose de reposo** cuadraba en 77/134… a costa de romper la
dinámica (§5.16: recalibrar daba 3/134 en reposo). La explicación es la de esta sesión: el
rectángulo estaba compensando la **composición ausente**. Con la composición correcta el dominio
vuelve a ser `[0,1]²`, como dice la especificación, y las dos poses cuadran a la vez.

Queda escrito para que nadie vuelva a buscar el rectángulo: no existe.

---

## 4. Lo que queda (menor, medido)

- **Extrapolación del perímetro.** Para puntos entre 1 y 3 unidades fuera de la rejilla, la
  especificación afina el perímetro con una base afín en lugar de extender las celdas. Aquí se usa
  la base afín directa; en estos modelos solo 9 vértices salen de `[0,1]²` y el error queda
  ≤ 6,5·10⁻⁴. Anotado en `crates/pose/src/deformar.rs`.
- **Flags de espejo**: se toman del primer form del conjunto (el de la izquierda), como dice la
  especificación. No se interpolan (son booleanos).

---

## 5. Estado del visor independiente (`docs/10` §8)

- **Geometría sin Core: ✅** 134/134 mallas con error < 2 px. Era el criterio exacto.
- **Render sin Core: ⬜ falta conectarlo.** El rasterizador CPU (`crates/raster`) y el motor
  (`crates/pose`) existen y están probados por separado; unirlos es un subcomando
  (`l2d render --motor`), no una incógnita.

Ese es el siguiente paso único, y por primera vez no depende de descubrir nada.

---

## 6. Cómo se reproduce

```bash
cargo test                                   # 35 pruebas + 1 doctest (13 de pose, 2 del marcador)
cargo run -p l2d -- pose models/samples/Samples/Resources/Hiyori/Hiyori.moc3 \
    --oraculo models/samples/hiyori.oracle.json
cargo run -p l2d -- pose …/Wanko.moc3 --oraculo models/samples/wanko.oracle.json
cargo run -p l2d -- pose …/Hiyori.moc3 --oraculo … --param ParamAngleZ=10   # pose movida
python3 tools/deform-completo.py             # el mismo algoritmo en Python, para contrastar
```

Los oráculos de poses movidas se generan con `models/samples/dump.html?p=ParamAngleZ=10`
(ver `docs/13` §5.5) y no se versionan, como los modelos.
