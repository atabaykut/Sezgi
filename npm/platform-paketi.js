#!/usr/bin/env node
'use strict';
// Hazır derlenmiş Sezgi ikilisinden platform paketi klasörü üretir (@sezgi/cli-<os>-<cpu>). Yayınlamaz.
// Kullanım: node platform-paketi.js <os-cpu> <ikili-yolu> <çıktı-klasörü>
//   ör. node platform-paketi.js win32-x64 ../../sezgi/target/release/sezgi.exe dist/cli-win32-x64
// Sürüm ana paketin sürümüdür: ana paket ile platform paketleri her zaman aynı sürümle çıkar.
const fs = require('fs');
const path = require('path');

const [anahtar, ikili, cikti] = process.argv.slice(2);
if (!anahtar || !ikili || !cikti) {
  console.error('Kullanım: node platform-paketi.js <os-cpu> <ikili-yolu> <çıktı-klasörü>');
  process.exit(2);
}
const ana = require('./sezgi/package.json');
const { platformlar } = require('./sezgi/platformlar.json');
const p = platformlar[anahtar];
if (!p) {
  console.error(`Bilinmeyen platform: ${anahtar} (bilinenler: ${Object.keys(platformlar).join(', ')})`);
  process.exit(2);
}
if (ana.optionalDependencies[p.paket] !== ana.version) {
  console.error(`Ana paket ${p.paket} için ${ana.optionalDependencies[p.paket]} istiyor, sürüm ${ana.version}: ikisi aynı olmalı`);
  process.exit(1);
}
const dosya = p.os === 'win32' ? 'sezgi.exe' : 'sezgi';
if (!fs.existsSync(ikili)) {
  console.error(`İkili yok: ${ikili}`);
  process.exit(1);
}
if (fs.existsSync(cikti) && fs.readdirSync(cikti).length) {
  console.error(`Çıktı klasörü boş değil: ${cikti}`);
  process.exit(1);
}
fs.mkdirSync(path.join(cikti, 'bin'), { recursive: true });
fs.copyFileSync(ikili, path.join(cikti, 'bin', dosya));
fs.chmodSync(path.join(cikti, 'bin', dosya), 0o755);
fs.copyFileSync(path.join(__dirname, 'sezgi', 'LICENSE'), path.join(cikti, 'LICENSE'));

const paket = {
  name: p.paket,
  version: ana.version,
  description: `Sezgi ${anahtar} ikilisi. Doğrudan kurulmaz: "npm install sezgi" bunu kendisi seçer.`,
  os: [p.os],
  cpu: [p.cpu],
  ...(p.libc ? { libc: [p.libc] } : {}),
  files: ['bin', 'LICENSE', 'README.md'],
  author: ana.author,
  license: ana.license,
  // Ana paketin başlatıcısı ikiliyi require.resolve ile bulur
  preferUnplugged: true,
};
fs.writeFileSync(path.join(cikti, 'package.json'), JSON.stringify(paket, null, 2) + '\n');
// macOS ikilileri MACOSX_DEPLOYMENT_TARGET=11.0 ile derlenir (sürüm iş akışı bunu Mach-O başlığından denetler)
const gereksinim = p.os === 'darwin' ? ' macOS 11 (Big Sur) ya da sonrası gerekir.' : '';
fs.writeFileSync(
  path.join(cikti, 'README.md'),
  `# ${p.paket}\n\nSezgi'nin ${anahtar} (${p.hedef}) için hazır derlenmiş ikilisi.${gereksinim} Doğrudan kurmana gerek yok:\n\n` +
  '```\nnpm install -g sezgi\n```\n\nana paketi bu platforma uyan ikiliyi kendisi seçer.\n'
);
console.log(`${p.paket}@${ana.version} → ${cikti}`);
