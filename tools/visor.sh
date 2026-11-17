#!/usr/bin/env bash
# Abre el visor propio (sin Core de Live2D).
#
# Uso:
#   tools/visor.sh                                   # Hiyori de muestra
#   tools/visor.sh ruta/al/modelo.moc3               # cualquier modelo
#
# Detalles del equipo: este escritorio carga el EGL de la NVIDIA antes que el de Mesa
# (`/etc/profile.d/nvidia-wayland.sh`), y con eso cualquier cliente EGL muere en
# `eglGetPlatformDisplay`. Forzar el vendor de Mesa lo arregla; es el mismo remedio que
# usan Nordend y Luciole en este equipo.
set -euo pipefail
cd "$(dirname "$0")/.."

MODELO="${1:-models/samples/Samples/Resources/Hiyori/Hiyori.moc3}"
if [ ! -f "$MODELO" ]; then
  echo "no existe el modelo: $MODELO" >&2
  echo "los modelos de muestra no se versionan (licencia de Live2D); ver docs/10 §9" >&2
  exit 1
fi

BIN="./target/release/l2d-ui"
if [ ! -x "$BIN" ]; then
  echo "compilando el visor (una vez)…" >&2
  cargo build --release -p l2d-ui
fi

export __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
exec "$BIN" "$MODELO"
