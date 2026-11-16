# 02 — Arquitectura

## Por qué Rust

| Criterio | Rust | C++ con el SDK oficial |
|---|---|---|
| Portabilidad Linux/Wayland | nativa, `wgpu` habla Vulkan/GL | buena, pero el SDK trae su propio render |
| Seguridad de memoria | sin UB en el parser de un formato binario ajeno | manual |
| Dependencias | `cargo`, reproducible | CMake + SDK + toolchain |
| Riesgo legal | cero líneas ajenas | enlaza código propietario de Live2D |
| Velocidad de arranque | alta (no hay SDK que aprender) | alta (el SDK ya hace todo) |

Se eligió Rust por la tercera fila: **no enlazar nada de Live2D** mantiene la independencia
demostrable. Si más adelante se prefiere el SDK oficial, el diseño lo permite (ver `ModelEngine`).

## Crates

```
l2d-viewer/
├── crates/moc3     contenedor .moc3: cabecera, canvas, conteos, tabla de secciones
├── crates/model3   model3.json: referencias, grupos, hit areas, movimientos
├── crates/raster   rasterizador CPU: triángulos, UV, blend modes, PNG
└── crates/l2d      CLI: inspect / model3 / demo
```

Dependencias externas totales: `serde`, `serde_json`, `png`. Nada más. Eso es deliberado: un
proyecto que va a durar años no debe arrastrar 300 crates.

## El pipeline que se está construyendo

```
bytes del .moc3
   │  moc3::Moc3::parse            ← ✅ hecho
   ▼
tablas del contenedor (offsets por sección)
   │  keyforms::load              ← fase 2
   ▼
modelo en reposo: mallas + deformadores + bindings de parámetros
   │  eval::apply(parámetros)     ← fase 2
   ▼
vértices deformados por drawable (posiciones, UVs, opacidad, orden)
   │  raster::triangle / wgpu     ← ✅ CPU, fase 3 GPU
   ▼
píxeles
```

## La frontera que hace el proyecto viable a largo plazo

Todo lo que "sabe de modelos" vive detrás de un trait:

```rust
pub trait ModelEngine {
    fn load(&mut self, moc: &[u8]) -> Result<(), EngineError>;
    fn set_parameter(&mut self, id: &str, value: f32);
    fn update(&mut self);
    fn drawables(&self) -> &[DrawableOut];
}
```

Dos implementaciones posibles, intercambiables:

| Implementación | Qué usa | Cuándo |
|---|---|---|
| `PureRust` | este repo | por defecto: independiente, sin binarios de Live2D |
| `CubismCoreFfi` | `libLive2DCubismCore.so` del SDK oficial | si se necesita paridad exacta o se quiere la vía bendecida |

El renderer no sabe cuál está detrás. Eso significa que si la evaluación propia no llega a
paridad, se cambia una línea y el resto del proyecto sigue sirviendo.

## Estado real del código (2026-11-13)

| Crate | Líneas | Pruebas | Estado |
|---|---:|---:|---|
| `moc3` | ~520 | 5 | contenedor completo, versiones 1–5 |
| `model3` | ~200 | 3 | manifiesto completo |
| `raster` | ~380 | 4 | triángulos, UV bilineal, 3 blend modes, PNG |
| `l2d` | ~200 | — | 3 subcomandos funcionando |

`cargo test` → **13 pruebas, todas verdes**. `l2d demo` produce un PNG de 640×640 con 1 152
triángulos deformados (verificado a ojo: la malla ondula y las UVs no se rompen).
