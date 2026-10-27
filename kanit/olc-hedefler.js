#!/usr/bin/env node
'use strict';
// 0.1 hedeflerinden ikisini ölçer (vizyon 15.3):
//   - Sezgi'nin açılışı, harita hazırken (< 100 ms): `sezgi mcp` süreci başlatılır, initialize ve ilk araç çağrısının
//     cevabı gelene kadar geçen süre. Süreç başlatma ve harita yükleme dahildir.
//   - Kart getirme ve yönlendirme p95 (< 5 ms): tek süreçte `tani` (kart getirme) ve `ara` (yönlendirme) çağrılarının
//     gidiş-dönüş süresi. Boru (stdio) gecikmesi dahildir; bu yüzden üst sınırdır.
// Kullanım: node olc-hedefler.js <sezgi-ikilisi> <harita-klasörü> [tekrar=1000]
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const [ikili, harita, tekrarArg] = process.argv.slice(2);
if (!ikili || !harita) {
  console.error('Kullanım: node olc-hedefler.js <sezgi-ikilisi> <harita-klasörü> [tekrar=1000]');
  process.exit(2);
}
const TEKRAR = Number(tekrarArg || 1000);
const kapilar = JSON.parse(fs.readFileSync(path.join(harita, 'kapilar.json'), 'utf8')).endpointler;

function sunucu() {
  const p = spawn(ikili, ['mcp', harita], { stdio: ['pipe', 'pipe', 'ignore'] });
  let tampon = '';
  const bekleyen = new Map();
  p.stdout.on('data', (d) => {
    tampon += d;
    let i;
    while ((i = tampon.indexOf('\n')) >= 0) {
      const satir = tampon.slice(0, i);
      tampon = tampon.slice(i + 1);
      const c = JSON.parse(satir);
      const b = bekleyen.get(c.id);
      if (b) { bekleyen.delete(c.id); b(c); }
    }
  });
  let id = 0;
  const cagir = (method, params) => new Promise((coz) => {
    id += 1;
    bekleyen.set(id, coz);
    p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  });
  return { p, cagir };
}

function yuzdelik(v, q) {
  const s = [...v].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.ceil(q * s.length) - 1)];
}

(async () => {
  // Açılış: 15 kez
  const acilis = [];
  for (let i = 0; i < 15; i++) {
    const t = process.hrtime.bigint();
    const { p, cagir } = sunucu();
    await cagir('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'olc', version: '0' } });
    await cagir('tools/call', { name: 'kimsin', arguments: {} });
    acilis.push(Number(process.hrtime.bigint() - t) / 1e6);
    p.stdin.end();
    await new Promise((r) => p.on('exit', r));
  }
  // Gecikme: tek süreçte, yarısı kart getirme (tani), yarısı yönlendirme (ara)
  const { p, cagir } = sunucu();
  await cagir('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'olc', version: '0' } });
  const sorgular = ['create', 'delete user', 'list', 'update settings', 'upload', 'login', 'search', 'album'];
  const tani = [], ara = [];
  let bos = 0;
  for (let i = 0; i < TEKRAR; i++) {
    const k = kapilar[i % kapilar.length];
    const t = process.hrtime.bigint();
    if (i % 2 === 0) {
      const c = await cagir('tools/call', { name: 'tani', arguments: { kapi: `${k.yontem} ${k.yol}` } });
      const metin = c.result.content[0].text;
      if (c.result.isError || !metin.includes(k.yol)) bos += 1;
      tani.push(Number(process.hrtime.bigint() - t) / 1e6);
    } else {
      await cagir('tools/call', { name: 'ara', arguments: { sorgu: sorgular[i % sorgular.length] } });
      ara.push(Number(process.hrtime.bigint() - t) / 1e6);
    }
  }
  p.stdin.end();
  const ms = (x) => `${x.toFixed(2)} ms`;
  const hepsi = tani.concat(ara);
  console.log(`Harita: ${kapilar.length} endpoint`);
  console.log(`Açılış (15 kez): ortanca ${ms(yuzdelik(acilis, 0.5))} · en kötü ${ms(Math.max(...acilis))}`);
  console.log(`Kart getirme (tani, ${tani.length}): p50 ${ms(yuzdelik(tani, 0.5))} · p95 ${ms(yuzdelik(tani, 0.95))}`);
  console.log(`Yönlendirme (ara, ${ara.length}): p50 ${ms(yuzdelik(ara, 0.5))} · p95 ${ms(yuzdelik(ara, 0.95))}`);
  console.log(`Kartı dolu gelmeyen tani cevabı: ${bos}`);
  console.log(`Hepsi (${hepsi.length}): p95 ${ms(yuzdelik(hepsi, 0.95))} · p99 ${ms(yuzdelik(hepsi, 0.99))}`);
})();
