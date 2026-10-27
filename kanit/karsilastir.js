// Sezgi'nin çıkardığı kapıları (kapilar.json) hakemin listesiyle karşılaştırır.
// Kullanım: node karsilastir.js <sezgi-kapilar.json> <hakem.json>
'use strict';
const fs = require('fs');
const path = require('path');
const sezgi = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const hakem = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));

const S = new Map(sezgi.endpointler.map(e => [e.id, e]));
const H = new Map(hakem.endpointler.map(e => [e.id, e]));
const alanlar = e => {
  const out = {};
  for (const g of [...e.govde, ...e.sorgu]) {
    if (!g.sema) continue;
    for (const [k, v] of Object.entries(g.sema.alanlar)) out[k] = { zorunlu: v.zorunlu, kurallar: [...v.kurallar].map(r => r.replace(/\s+/g, ' ').replace(/\(\)$/, '()')).sort() };
  }
  return out;
};
const norm = a => [...a].map(x => String(x).replace(/^UserRole./, '')).sort().join(',');

const sayac = { bulunan: 0, eksik: [], fazla: [], giris: 0, roller: 0, girdi_alan: 0, zorunluluk: 0, kural: 0, alan_toplam: 0, kural_toplam: 0, farklar: [] };
for (const [id, h] of H) {
  const s = S.get(id);
  if (!s) { sayac.eksik.push(id); continue; }
  sayac.bulunan++;
  if (s.giris === h.giris) sayac.giris++; else sayac.farklar.push(`${id} giris: sezgi=${s.giris} hakem=${h.giris}`);
  if (norm(s.roller) === norm(h.roller)) sayac.roller++; else sayac.farklar.push(`${id} roller: sezgi=${s.roller} hakem=${h.roller}`);
  const ha = alanlar(h), sa = alanlar(s);
  for (const [k, v] of Object.entries(ha)) {
    sayac.alan_toplam++;
    sayac.kural_toplam += v.kurallar.length;
    const sv = sa[k];
    if (!sv) { sayac.farklar.push(`${id} alan yok: ${k}`); continue; }
    sayac.girdi_alan++;
    if (sv.zorunlu === v.zorunlu) sayac.zorunluluk++; else sayac.farklar.push(`${id} ${k} zorunlu: sezgi=${sv.zorunlu} hakem=${v.zorunlu}`);
    const sk = new Set(sv.kurallar.map(r => r.replace(/\s/g, '')));
    const eslesen = v.kurallar.filter(r => sk.has(r.replace(/\s/g, ''))).length;
    sayac.kural += eslesen;
    if (eslesen < v.kurallar.length) sayac.farklar.push(`${id} ${k} kural: sezgi=[${sv.kurallar}] hakem=[${v.kurallar}]`);
  }
}
for (const id of S.keys()) if (!H.has(id)) sayac.fazla.push(id);

const yuzde = (a, b) => (b ? ((100 * a) / b).toFixed(1) + '%' : '-');
console.log(`Endpoint bulma      ${sayac.bulunan}/${H.size}  (${yuzde(sayac.bulunan, H.size)})`);
console.log(`Yanlış/fazla kapı   ${sayac.fazla.length}`);
console.log(`Giriş durumu        ${yuzde(sayac.giris, sayac.bulunan)}`);
console.log(`Roller              ${yuzde(sayac.roller, sayac.bulunan)}`);
console.log(`Girdi alanı bulma   ${sayac.girdi_alan}/${sayac.alan_toplam}  (${yuzde(sayac.girdi_alan, sayac.alan_toplam)})`);
console.log(`Zorunluluk doğru    ${yuzde(sayac.zorunluluk, sayac.girdi_alan)}`);
console.log(`Doğrulama kuralları ${sayac.kural}/${sayac.kural_toplam}  (${yuzde(sayac.kural, sayac.kural_toplam)})`);
if (sayac.eksik.length) console.log('\nEksik:', sayac.eksik.slice(0, 15).join(' | '));
if (sayac.fazla.length) console.log('Fazla:', sayac.fazla.slice(0, 15).join(' | '));
if (sayac.farklar.length) console.log(`\nİlk farklar (${sayac.farklar.length}):\n  ` + sayac.farklar.slice(0, 25).join('\n  '));
