#!/usr/bin/env bash
# Abre el editor (prototipo) en el navegador.
# Sirve models/samples por HTTP porque el Core necesita fetch() del .moc3.
set -euo pipefail
cd "$(dirname "$0")/../models/samples"
MODEL="${1:-Hiyori}"
PORT="${PORT:-8770}"
python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 &
SRV=$!
trap 'kill $SRV 2>/dev/null || true' EXIT
sleep 0.6
URL="http://127.0.0.1:${PORT}/editor.html?m=${MODEL}"
echo "editor: $URL"
xdg-open "$URL" 2>/dev/null || echo "abre esa URL en el navegador"
wait $SRV
