// Benlik Kartlarını standardın JSON şemasına karşı doğrular (ajv 2020-12).
// Kullanım: AJV_YOLU=<node_modules klasörü> node kart-dogrula.js <şema.json> <kart.json>...
'use strict';
const fs = require('fs');
const path = require('path');
const kok = process.env.AJV_YOLU || 'node_modules';
const Ajv = require(path.join(kok, 'ajv', 'dist', '2020')).default;
const ajv = new Ajv({ allErrors: true, strict: false });
require(path.join(kok, 'ajv-formats'))(ajv);
const dogrula = ajv.compile(JSON.parse(fs.readFileSync(process.argv[2], 'utf8')));
let hata = 0;
for (const f of process.argv.slice(3)) {
  const ok = dogrula(JSON.parse(fs.readFileSync(f, 'utf8')));
  console.log(`${ok ? '✓' : '✗'} ${f}`);
  if (!ok) {
    hata++;
    for (const e of dogrula.errors.slice(0, 8)) console.log(`   ${e.instancePath} ${e.message}`);
  }
}
process.exit(hata ? 1 : 0);
