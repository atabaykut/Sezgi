// Sezgi'nin kapilar.json çıktısını bir uygulamanın resmî OpenAPI dokümanıyla karşılaştırır.
// Kullanım: node karsilastir-openapi.js <sezgi-kapilar.json> <openapi.json> [--onek /api]
'use strict';
const fs = require('fs');
const sezgi = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const doc = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const i = process.argv.indexOf('--onek');
const onek = i > 0 ? process.argv[i + 1] : '';
const S = (doc.components && doc.components.schemas) || {};
const norm = y => y.replace(/\{[^}]+\}|:[A-Za-z0-9_]+/g, '{}').replace(/\/+$/, '').toLowerCase();

const resmi = new Map();
for (const [p, ops] of Object.entries(doc.paths)) {
  for (const [m, o] of Object.entries(ops)) {
    if (!o || typeof o !== 'object' || !o.responses) continue;
    // Gövde doğrudan ya da `#/components/requestBodies/X` üzerinden şemaya bağlanır
    let govde = o.requestBody;
    if (govde && govde.$ref) govde = ((doc.components || {}).requestBodies || {})[govde.$ref.split('/').pop()];
    const ref = govde && govde.content && Object.values(govde.content)[0].schema;
    const sema = ref && ref.$ref ? S[ref.$ref.split('/').pop()] : ref && ref.properties ? ref : null;
    resmi.set(`${m.toUpperCase()} ${norm(onek + p)}`, { o, sema });
  }
}
// Aynı adrese birden fazla kapı düşebilir: ikincil uygulama ya da farklı API sürümü.
// Resmî doküman ana uygulamayı ve (varsa sürüm başlığıyla) belirli bir sürümü anlatır; o kapı seçilir.
const adaylar = new Map();
for (const e of sezgi.endpointler) {
  const k = `${e.yontem} ${norm(e.yol)}`;
  (adaylar.get(k) || adaylar.set(k, []).get(k)).push(e);
}
const belgeSurumu = o => {
  const p = (o.parameters || []).find(q => q.in === 'header' && /api-version/i.test(q.name));
  if (!p) return null;
  return (p.schema && p.schema.default) || ((p.description || '').match(/\d{4}-\d{2}-\d{2}/) || [])[0] || null;
};
const sz = new Map();
for (const [k, liste] of adaylar) {
  const r = resmi.get(k);
  const surum = r && belgeSurumu(r.o);
  const ana = liste.filter(e => e.ana !== false);
  const secilen = (surum && ana.find(e => (e.surum || '').split(',').includes(surum))) || ana[ana.length - 1] || liste[0];
  sz.set(k, secilen);
}

// İşlemin kendi `security`'si yoksa belgenin genel `security`'si geçerlidir (OpenAPI 3)
const islemSecurity = o => (o.security !== undefined ? o.security : doc.security || []);
const belgeSecurityKullaniyor = [...resmi.values()].some(r => islemSecurity(r.o).length > 0);
let bulunan = 0, giris = 0, alanToplam = 0, alanBulunan = 0, zorunlu = 0, govdeli = 0;
const eksik = [], farklar = [];
for (const [k, r] of resmi) {
  const e = sz.get(k);
  if (!e) { eksik.push(k); continue; }
  bulunan++;
  // Doküman kimliği iki biçimde anlatabilir: `security` alanı ya da Authorization gibi başlık parametresi.
  const kimlikBasligi = (r.o.parameters || []).find(q => q.in === 'header' && /^(authorization|x-[\w-]*(secret|api)[\w-]*key)$/i.test(q.name));
  let resmiGiris, sezgiGiris;
  if (belgeSecurityKullaniyor) {
    // `security` boşsa ya da boş bir seçenek ({}) içeriyorsa kimliksiz istek de geçer. Sezgi'nin "isteğe bağlı" kararı
    // da kimliksiz isteğin geçtiğini söyler: ikisi de "herkese açık" sayılır.
    const sec = islemSecurity(r.o);
    resmiGiris = sec.length > 0 && !sec.some(x => x && Object.keys(x).length === 0) ? 'gerekli' : 'herkese_acik';
    sezgiGiris = e.giris === 'gerekli' ? 'gerekli' : 'herkese_acik';
  } else {
    resmiGiris = !kimlikBasligi ? 'herkese_acik' : kimlikBasligi.required ? 'gerekli' : 'istege_bagli';
    sezgiGiris = e.giris;
  }
  if (resmiGiris === sezgiGiris) giris++; else farklar.push(`${k} giriş: sezgi=${e.giris} resmi=${resmiGiris}`);
  if (r.sema && r.sema.properties) {
    govdeli++;
    const sAlan = {};
    for (const g of e.govde) if (g.sema) Object.assign(sAlan, g.sema.alanlar);
    for (const [ad] of Object.entries(r.sema.properties)) {
      alanToplam++;
      if (!sAlan[ad]) { farklar.push(`${k} alan yok: ${ad}`); continue; }
      alanBulunan++;
      const rz = (r.sema.required || []).includes(ad);
      if (rz === sAlan[ad].zorunlu) zorunlu++; else farklar.push(`${k} ${ad} zorunlu: sezgi=${sAlan[ad].zorunlu} resmi=${rz}`);
    }
  }
}
const fazla = [...sz.keys()].filter(k => !resmi.has(k));
const y = (a, b) => (b ? ((100 * a) / b).toFixed(1) + '%' : '-');
console.log(`Resmî dokümandaki işlem     ${resmi.size}`);
console.log(`Sezgi'nin bulduğu           ${bulunan}/${resmi.size} (${y(bulunan, resmi.size)})`);
console.log(`Sezgi'de olup dokümanda yok ${fazla.length}`);
console.log(`Giriş durumu doğru          ${y(giris, bulunan)}`);
console.log(`Gövde alanı bulma           ${alanBulunan}/${alanToplam} (${y(alanBulunan, alanToplam)})`);
console.log(`Zorunluluk doğru            ${y(zorunlu, alanBulunan)}`);
if (eksik.length) console.log(`\nEksik (${eksik.length}): ` + eksik.slice(0, 12).join(' | '));
if (fazla.length) console.log(`Fazla (${fazla.length}): ` + fazla.slice(0, 12).join(' | '));
const tum = process.argv.includes('--tum');
if (farklar.length) console.log(`\n${tum ? 'Bütün' : 'İlk'} farklar (${farklar.length}):\n  ` + (tum ? farklar : farklar.slice(0, 20)).join('\n  '));
