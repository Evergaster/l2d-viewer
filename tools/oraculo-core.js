// Oráculo: usa el Core OFICIAL de Live2D (Web) para obtener la pose por defecto real.
// Solo lectura local; no se redistribuye nada de Live2D.
const fs = require('fs');
const path = require('path');

const CORE = '/tmp/core.js';
const code = fs.readFileSync(CORE, 'utf8');
// El min.js es UMD; se evalúa en un contexto con globalThis.
const vm = require('vm');
const sandbox = { console, globalThis: null, self: undefined, window: undefined };
sandbox.globalThis = sandbox;
vm.createContext(sandbox);
try {
  vm.runInContext(code, sandbox, { filename: 'core.js' });
} catch (e) {
  console.error('eval core:', e.message);
}
const Core = sandbox.Live2DCubismCore || sandbox.window?.Live2DCubismCore;
if (!Core) {
  console.error('no Live2DCubismCore en el contexto; claves:', Object.keys(sandbox).slice(0, 20));
  process.exit(1);
}
console.log('core version:', Core.Version?.csmGetVersion ? 'api' : 'ok', Core.Version?.csmGetVersion?.());

const file = process.argv[2];
const buf = fs.readFileSync(file);
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);

const moc = Core.Moc.fromArrayBuffer(ab);
console.log('moc version:', moc._version ?? 'n/a');
const model = moc.instantiateModel();
model.update();

const d = model.drawables;
console.log('drawables:', d.count);
console.log('vertexCounts[:6]:', Array.from(d.vertexCounts.slice(0, 6)));
console.log('indexCounts[:6]:', Array.from(d.indexCounts.slice(0, 6)));
console.log('textureIndices[:6]:', Array.from(d.textureIndices.slice(0, 6)));
console.log('drawOrders[:6]:', Array.from(d.drawOrders.slice(0, 6)));

// primer drawable: primeros vértices de la pose por defecto
const vp = d.vertexPositions;
const n0 = d.vertexCounts[0];
const first = [];
for (let i = 0; i < 8; i++) first.push([+vp[i * 2].toFixed(3), +vp[i * 2 + 1].toFixed(3)]);
console.log('drawable0 verts[:8]:', JSON.stringify(first));
console.log('drawable0 count:', n0, 'bbox:',
  (() => {
    let minx = 1e9, miny = 1e9, maxx = -1e9, maxy = -1e9;
    for (let i = 0; i < n0; i++) {
      minx = Math.min(minx, vp[i * 2]); maxx = Math.max(maxx, vp[i * 2]);
      miny = Math.min(miny, vp[i * 2 + 1]); maxy = Math.max(maxy, vp[i * 2 + 1]);
    }
    return [minx.toFixed(1), miny.toFixed(1), maxx.toFixed(1), maxy.toFixed(1)];
  })());

// parámetros por defecto
console.log('params:', model.parameters.count, 'ids[:4]:', Array.from(model.parameters.ids.slice(0, 4)));
console.log('defaults[:6]:', Array.from(model.parameters.defaultValues.slice(0, 6)));

// suma de vértices de todos los drawables (para ver si están en espacio de canvas)
let minx = 1e9, miny = 1e9, maxx = -1e9, maxy = -1e9, off = 0;
for (let i = 0; i < d.count; i++) {
  const n = d.vertexCounts[i];
  for (let k = 0; k < n; k++) {
    const x = vp[off + k * 2], y = vp[off + k * 2 + 1];
    minx = Math.min(minx, x); maxx = Math.max(maxx, x);
    miny = Math.min(miny, y); maxy = Math.max(maxy, y);
  }
  off += n * 2;
}
console.log('bbox global por defecto:', [minx.toFixed(1), miny.toFixed(1), maxx.toFixed(1), maxy.toFixed(1)]);
