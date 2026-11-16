#!/usr/bin/env bash
# Abre el editor (prototipo) en el navegador.
# Sirve la RAÍZ del repo por HTTP porque el Core necesita fetch() del .moc3,
# y los modelos viven en models/samples (no versionado). Editor: editor/editor.html.
set -euo pipefail
cd "$(dirname "$0")/.."
MODEL="${1:-Hiyori}"
PORT="${PORT:-8770}"
MOC="models/samples/Samples/Resources/$MODEL/$MODEL.moc3"
if [ ! -f "$MOC" ]; then
  echo "falta $MOC" >&2
  echo "los modelos de muestra no están en el repo: copia el SDK en models/samples (docs/10 §9)" >&2
  exit 1
fi
python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 &
SRV=$!
trap 'kill $SRV 2>/dev/null || true' EXIT
sleep 0.6
URL="http://127.0.0.1:${PORT}/editor/editor.html?m=${MODEL}"
echo "editor: $URL"
xdg-open "$URL" 2>/dev/null || echo "abre esa URL en el navegador"
wait $SRV
