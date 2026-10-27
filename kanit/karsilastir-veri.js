// Sezgi'nin veri bağlarını (kapilar.json içindeki `veri`) veri hakeminin sonucuyla karşılaştırır.
// Kullanım: node karsilastir-veri.js <hakem-veri.json> <kapilar.json>
'use strict';
const fs = require('fs');
const hakem = JSON.parse(fs.readFileSync(process.argv[2], 'utf8')).metotlar;
const kapilar = JSON.parse(fs.readFileSync(process.argv[3], 'utf8')).endpointler;

const sezgi = {};
for (const k of kapilar) sezgi[`${k.kontrolcu}.${k.fonksiyon}`] = k.veri || { okur: [], yazar: [] };

let [dp, yp, yn, tam, metot] = [0, 0, 0, 0, 0];
const farklar = [];
for (const [ad, h] of Object.entries(hakem)) {
  const s = sezgi[ad];
  if (!s) continue;
  metot++;
  let ayni = true;
  for (const tur of ['okur', 'yazar']) {
    const hs = new Set(h[tur]), ss = new Set(s[tur]);
    for (const m of ss) (hs.has(m) ? dp++ : (yp++, (ayni = false), farklar.push(`${ad} ${tur} fazla: ${m}`)));
    for (const m of hs) if (!ss.has(m)) (yn++, (ayni = false), farklar.push(`${ad} ${tur} eksik: ${m}`));
  }
  if (ayni) tam++;
}
const kesinlik = dp / (dp + yp || 1), duyarlilik = dp / (dp + yn || 1);
console.log(`Metot ${metot} · birebir aynı ${tam} (%${((100 * tam) / metot).toFixed(1)})`);
console.log(`Bağ: doğru ${dp} · fazla ${yp} · eksik ${yn} · kesinlik %${(100 * kesinlik).toFixed(1)} · duyarlılık %${(100 * duyarlilik).toFixed(1)}`);
for (const f of farklar.slice(0, 40)) console.log('  ' + f);
if (farklar.length > 40) console.log(`  … ${farklar.length - 40} fark daha`);
