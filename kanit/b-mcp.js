// B düzeni: Uygulamanın kendi OpenAPI (Swagger) dokümanını, Sezgi'yle BİREBİR AYNI araç arayüzüyle
// (kimsin, ara, tani) sunan MCP sunucusu. Adil karşılaştırma için tek fark bilgi kaynağıdır.
// Kullanım: node b-mcp.js <openapi.json>
'use strict';
const fs = require('fs');
const readline = require('readline');
const doc = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const S = (doc.components && doc.components.schemas) || {};

const kelimeler = s => (s || '').replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase().split(/[^a-z0-9ğüşıöç]+/i).filter(Boolean);
const coz = sch => (sch && sch.$ref ? S[sch.$ref.split('/').pop()] || {} : sch || {});
const islemler = [];
for (const [yol, ops] of Object.entries(doc.paths)) {
  for (const [y, o] of Object.entries(ops)) {
    const yontem = y.toUpperCase();
    const kimlik = o.operationId || '';
    const govdeRef = o.requestBody && o.requestBody.content && Object.values(o.requestBody.content)[0].schema;
    const govdeAd = govdeRef && govdeRef.$ref ? govdeRef.$ref.split('/').pop() : null;
    const govde = coz(govdeRef);
    islemler.push({
      yontem, yol: yol.replace(/\{(\w+)\}/g, ':$1'), kimlik, ozet: o.summary || o.description || '',
      etiket: (o.tags || []).join(','), guvenlik: (o.security || []).length > 0,
      parametreler: (o.parameters || []).filter(p => p.in === 'path').map(p => p.name),
      sorgu: (o.parameters || []).filter(p => p.in === 'query').map(p => ({ ad: p.name, zorunlu: !!p.required, sema: p.schema || {} })),
      govdeAd, govde,
    });
  }
}
const satir = i => `${i.yontem} ${i.yol} · ${i.kimlik}${i.ozet ? ' · ' + i.ozet : ''} · giriş:${i.guvenlik ? 'gerekli' : 'herkese_acik'}`;
const kurallar = p => ['minLength', 'maxLength', 'minimum', 'maximum', 'pattern', 'format', 'enum', 'minItems', 'maxItems'].filter(k => p[k] !== undefined).map(k => `${k}=${JSON.stringify(p[k])}`).join(' ');

function kimsin() {
  const m = {};
  for (const i of islemler) (m[i.etiket || '-'] = m[i.etiket || '-'] || []).push(i.yol);
  return `Uygulama: ${doc.info.title} · ${islemler.length} işlem (OpenAPI)\nEtiketler (ad · işlem sayısı):\n` + Object.entries(m).map(([k, v]) => `  ${k} · ${v.length}`).join('\n');
}
function ara(sorgu, adet = 8) {
  const q = kelimeler(sorgu);
  const p = islemler.map(i => {
    const yol = kelimeler(i.yol), kim = kelimeler(i.kimlik), oz = kelimeler(i.ozet), et = kelimeler(i.etiket), alan = Object.keys(i.govde.properties || {}).flatMap(kelimeler);
    const es = l => q.reduce((t, w) => t + (l.some(x => x === w || (w.length > 3 && x.length > 3 && (x.startsWith(w) || w.startsWith(x)))) ? 1 : 0), 0);
    return [3 * es(yol) + 3 * es(kim) + 2 * es(oz) + 1.5 * es(et) + es(alan), i];
  }).filter(x => x[0] > 0).sort((a, b) => b[0] - a[0]);
  if (!p.length) return `'${sorgu}' için işlem bulunamadı.`;
  return p.slice(0, adet).map(x => satir(x[1])).join('\n') + (p.length > adet ? `\n(+${p.length - adet} daha)` : '');
}
function tani(k) {
  const [yontem, ...r] = k.trim().split(' ');
  const norm = y => y.trim().replace(/\/$/, '').split('/').map(s => (s.startsWith(':') || s.startsWith('{') ? '{}' : s)).join('/').toLowerCase();
  const i = islemler.find(x => x.yontem === (yontem || '').toUpperCase() && norm(x.yol) === norm(r.join(' ')));
  if (!i) return `'${k}' bulunamadı. Biçim: "POST /api/v1/ornek/:id".`;
  let s = satir(i) + '\n';
  if (i.parametreler.length) s += `yol parametreleri: ${i.parametreler.join(', ')}\n`;
  if (i.govdeAd) {
    s += `gövde (${i.govdeAd}):\n`;
    const props = i.govde.properties || {};
    if (!Object.keys(props).length) s += '  (dokümanda alan tanımı yok)\n';
    for (const [ad, p] of Object.entries(props)) s += `  ${ad}${(i.govde.required || []).includes(ad) ? ' *zorunlu' : ' ?isteğe bağlı'} ${p.type || (p.$ref ? p.$ref.split('/').pop() : '?')}${kurallar(p) ? ' · ' + kurallar(p) : ''}${p.description ? ' · ' + p.description : ''}\n`;
  }
  for (const q of i.sorgu) s += `sorgu: ${q.ad}${q.zorunlu ? ' *zorunlu' : ''} ${q.sema.type || ''}\n`;
  return s;
}

const ARACLAR = [
  { name: 'kimsin', description: 'Uygulamanın özeti: etiketler ve işlem sayıları.', inputSchema: { type: 'object', properties: {} } },
  { name: 'ara', description: "İsteğe uyan işlemleri (endpoint) bulur. Kodun dilindeki (çoğunlukla İngilizce) anahtar kelimelerle ara.", inputSchema: { type: 'object', properties: { sorgu: { type: 'string' }, adet: { type: 'integer' } }, required: ['sorgu'] } },
  { name: 'tani', description: 'Bir işlemin ayrıntısı: yol parametreleri, gövde ve sorgu alanları, zorunluluk ve kısıtlar.', inputSchema: { type: 'object', properties: { kapi: { type: 'string' } }, required: ['kapi'] } },
];
const TALIMAT = "Bu sunucu bir uygulamanın OpenAPI dokümanıdır. Önce `ara` ile isteğe uyan işlemleri bul, sonra `tani` ile ayrıntısını öğren. Yol parametreleri :ad biçimindedir. Türkçe isteği İngilizce anahtar kelimelere çevirerek ara.";

const rl = readline.createInterface({ input: process.stdin });
rl.on('line', line => {
  if (!line.trim()) return;
  let m; try { m = JSON.parse(line); } catch { return; }
  if (m.id === undefined) return;
  let result;
  if (m.method === 'initialize') result = { protocolVersion: (m.params || {}).protocolVersion || '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'openapi-b', version: '0.1' }, instructions: TALIMAT };
  else if (m.method === 'tools/list') result = { tools: ARACLAR };
  else if (m.method === 'tools/call') {
    const a = m.params.arguments || {};
    const t = m.params.name === 'kimsin' ? kimsin() : m.params.name === 'ara' ? ara(a.sorgu || '', a.adet || 8) : m.params.name === 'tani' ? tani(a.kapi || '') : 'Bilinmeyen araç';
    result = { content: [{ type: 'text', text: t }], isError: false };
  } else if (m.method === 'ping') result = {};
  else { process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: m.id, error: { code: -32601, message: 'Method not found' } }) + '\n'); return; }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: m.id, result }) + '\n');
});
