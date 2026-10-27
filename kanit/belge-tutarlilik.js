#!/usr/bin/env node
// Belge tutarlılık denetimi: README'lerdeki ölçüm sayıları kaynak dosyalarıyla aynı mı?
// belge-sayilari.json içindeki her satır için belgede metnin geçtiği ve kaynakta desenin bulunduğu denetlenir.
// Uyuşmazlıkta çıkış kodu 1. Kaynağı yayınlanmayan sayılar ("dogrulanamayan") ayrıca listelenir, sessizce geçilmez.
// Doc consistency check: every benchmark number in the READMEs must match its source file.
'use strict';
const fs = require('fs');
const path = require('path');

// Yayınlanan depoda kanit/ Sezgi'nin kökündedir; geliştirme düzeninde kanit/ ile sezgi/ yan yanadır
const kanit = __dirname;
const yayinda = fs.existsSync(path.join(kanit, '..', 'Cargo.toml'));
const kok = yayinda ? path.join(kanit, '..') : path.join(kanit, '..', 'sezgi');
const yol = (p) => (p.startsWith('kanit/') ? path.join(kanit, p.slice('kanit/'.length)) : path.join(kok, p));
const oku = (p) => fs.readFileSync(yol(p), 'utf8');

const liste = JSON.parse(fs.readFileSync(path.join(kanit, 'belge-sayilari.json'), 'utf8'));
let hata = 0;
for (const s of liste.sayilar) {
  const belge = oku(s.belge);
  if (!belge.includes(s.metin)) {
    console.log(`✗ ${s.belge}: metin yok: ${s.metin}`);
    hata++;
    continue;
  }
  if (!new RegExp(s.desen).test(oku(s.kaynak))) {
    console.log(`✗ ${s.belge}: "${s.metin}" kaynağında (${s.kaynak}) bulunamadı`);
    hata++;
  }
}
for (const d of liste.dogrulanamayan) {
  if (oku(d.belge).includes(d.metin)) console.log(`! ${d.belge}: "${d.metin}" doğrulanamaz: ${d.neden}`);
}
// Belgelerde süre (ms) yazıyorsa kaynağı listede olmalı: kaynaksız süre yazılmaz
for (const b of ['README.md', 'README.tr.md']) {
  const kaynakli = liste.sayilar.filter((s) => s.belge === b).map((s) => s.metin).join('\n');
  for (const m of oku(b).matchAll(/\|[^|\n]*\b\d[\d.,]*\s?ms\b[^|\n]*\|/g)) {
    if (!kaynakli.includes(m[0])) {
      console.log(`✗ ${b}: kaynağı listede olmayan süre: ${m[0]}`);
      hata++;
    }
  }
  // Satır sayısı (ör. "61,567 lines") da kaynaklı olmalı
  for (const m of oku(b).matchAll(/\b\d{1,3}(?:[.,]\d{3})+ (?:lines|satır)\b/g)) {
    if (!kaynakli.includes(m[0])) {
      console.log(`✗ ${b}: kaynağı listede olmayan satır sayısı: ${m[0]}`);
      hata++;
    }
  }
}
// Açık kanıtın Node gereksinimi: kilit dosyasındaki en yüksek "engines.node" alt sınırı package.json'da zorunlu
// (engines + .npmrc engine-strict) ve README'de aynı sürümle yazılı olmalı
{
  const acik = path.join(kanit, 'acik');
  const surum = (s) => (String(s).match(/(\d+)(?:\.(\d+))?/) || []).slice(1).map((x) => Number(x || 0));
  const buyuk = (a, b) => a[0] - b[0] || a[1] - b[1];
  const kilit = JSON.parse(fs.readFileSync(path.join(acik, 'package-lock.json'), 'utf8'));
  const enYuksek = Object.values(kilit.packages)
    .map((p) => p.engines && p.engines.node)
    .filter(Boolean)
    .map(surum)
    .sort(buyuk)
    .pop();
  const paket = JSON.parse(fs.readFileSync(path.join(acik, 'package.json'), 'utf8'));
  const istenen = surum((paket.engines || {}).node || '0');
  const npmrc = fs.existsSync(path.join(acik, '.npmrc')) ? fs.readFileSync(path.join(acik, '.npmrc'), 'utf8') : '';
  const readme = fs.readFileSync(path.join(acik, 'README.md'), 'utf8');
  const yazili = readme.match(/Node\.js (\d+(?:\.\d+)?)\+/);
  if (!paket.engines || buyuk(istenen, enYuksek) < 0) {
    console.log(`✗ kanit/acik/package.json engines.node (${(paket.engines || {}).node}) bağımlılıkların istediğinden (${enYuksek.join('.')}) düşük`);
    hata++;
  }
  if (!/engine-strict\s*=\s*true/.test(npmrc)) {
    console.log('✗ kanit/acik/.npmrc engine-strict=true değil: engines zorunlu değil');
    hata++;
  }
  if (!yazili || buyuk(surum(yazili[1]), istenen) !== 0) {
    console.log(`✗ kanit/acik/README.md Node gereksinimi (${yazili ? yazili[1] : 'yok'}) engines ile (${istenen.join('.')}) aynı değil`);
    hata++;
  }
}
console.log(hata ? `✗ ${hata} uyuşmazlık` : `✓ ${liste.sayilar.length} sayı kaynağıyla aynı`);
process.exit(hata ? 1 : 0);
