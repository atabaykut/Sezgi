// Hesap ipuçları ölçümü (ON-KAYIT-5): haritadaki her uç için MCP `tani` çağrılır, "Hesap ipuçları" satırları toplanır
// ve ön kayıttaki hakem tablosuyla karşılaştırılır. Uygulama çalıştırılmaz.
// Kullanım: node hesap-ipuclari-olc.js <sezgi-ikilisi> <harita-klasörü>
'use strict';
const { spawn } = require('child_process');
const path = require('path');
const readline = require('readline');

const [sezgi, harita] = process.argv.slice(2);
const uclar = require(path.resolve(harita, 'kapilar.json')).endpointler;

// Ön kayıttaki hakem (docs/api_doc_en.md)
const HESAP = ['performance_selection', 'aspect_ratios_selection', 'image_number', 'base_model_name', 'refiner_model_name', 'uov_method', 'upscale_value'];
const HAFIF = [['performance_selection', 'Speed'], ['performance_selection', 'Extreme Speed'], ['uov_method', 'Upscale (Fast 2x)']];

const p = spawn(sezgi, ['mcp', harita], { stdio: ['pipe', 'pipe', 'inherit'] });
const bekleyen = new Map();
readline.createInterface({ input: p.stdout }).on('line', l => {
  let m;
  try { m = JSON.parse(l); } catch { return; }
  const b = bekleyen.get(m.id);
  if (b) { bekleyen.delete(m.id); b(m); }
});
let sira = 0;
const iste = (method, params) => new Promise(r => { const id = ++sira; bekleyen.set(id, r); p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n'); });

(async () => {
  await iste('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'hesap-olc', version: '1' } });
  const ipuclari = [];
  for (const u of uclar) {
    const c = await iste('tools/call', { name: 'tani', arguments: { kapi: u.id } });
    const metin = (((c.result || {}).content || [])[0] || {}).text || '';
    const i = metin.indexOf('Hesap ipuçları');
    if (i < 0) continue;
    for (const s of metin.slice(i).split('\n').slice(1)) {
      if (!s.startsWith('  - ')) break;
      ipuclari.push({ uc: u.id, satir: s.slice(4) });
    }
  }
  p.stdin.end();
  const alanli = ipuclari.map(x => ({ ...x, alan: (x.satir.match(/^`([^`]+)`/) || [])[1] || null }));
  const bulunan = HESAP.filter(a => alanli.some(x => x.alan === a));
  const hafif = HAFIF.filter(([a, d]) => alanli.some(x => x.alan === a && x.satir.includes('hafif seçenek') && x.satir.split('(bütün')[0].includes(d)));
  const yanlis = alanli.filter(x => x.alan && !HESAP.includes(x.alan));
  console.log(JSON.stringify({
    uc: uclar.length,
    ipucu_verilen_uc: new Set(ipuclari.map(x => x.uc)).size,
    ipucu_satiri: ipuclari.length,
    bulma: `${bulunan.length}/${HESAP.length}`,
    bulunan,
    hafif: `${hafif.length}/${HAFIF.length}`,
    yanlis: yanlis.length,
    yanlis_alanlar: [...new Set(yanlis.map(x => x.alan))],
    ipuclari,
  }, null, 2));
})();
