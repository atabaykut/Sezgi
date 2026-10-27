#!/usr/bin/env node
'use strict';
// Sezgi başlatıcısı: npm, bu işletim sistemine uyan hazır derlenmiş paketi (@sezgi/cli-<os>-<cpu>) isteğe bağlı
// bağımlılık olarak kurar. Bu dosya yalnızca o ikiliyi bulup aynı argümanlarla çalıştırır; kendisi hiçbir iş yapmaz.
const { spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { platformlar } = require('../platformlar.json');

function ikiliYolu() {
  // Kendi derlediğin ikiliyi kullanmak için: SEZGI_IKILI=/yol/sezgi
  if (process.env.SEZGI_IKILI) return process.env.SEZGI_IKILI;
  const anahtar = `${process.platform}-${process.arch}`;
  const p = platformlar[anahtar];
  if (!p) {
    throw new Error(
      `Sezgi bu platform için hazır derlenmiş olarak gelmiyor: ${anahtar}.\n` +
      `Desteklenenler: ${Object.keys(platformlar).join(', ')}.\n` +
      'Kaynaktan derleyebilirsin (cargo build --release), sonra SEZGI_IKILI ile yolunu ver.'
    );
  }
  // macOS ikilileri macOS 11 (Big Sur, Darwin 20) ve sonrası için derlenir; daha eskisinde yükleyici anlaşılmaz bir
  // hatayla durur. Darwin çekirdek sürümünden önceden söylenir.
  if (process.platform === 'darwin' && parseInt(os.release().split('.')[0], 10) < 20) {
    throw new Error('macOS paketleri macOS 11 (Big Sur) ya da sonrasını gerektirir. Kaynaktan derle (cargo build --release) ve SEZGI_IKILI ile yolunu ver.');
  }
  if (process.platform === 'linux' && process.report && process.report.getReport) {
    const r = process.report.getReport();
    if (r.header && !r.header.glibcVersionRuntime) {
      throw new Error('Linux paketleri glibc gerektirir (Alpine/musl desteklenmiyor). Kaynaktan derle (cargo build --release) ve SEZGI_IKILI ile yolunu ver.');
    }
  }
  const dosya = process.platform === 'win32' ? 'sezgi.exe' : 'sezgi';
  try {
    return require.resolve(`${p.paket}/bin/${dosya}`);
  } catch {
    throw new Error(
      `${p.paket} kurulmamış. npm isteğe bağlı bağımlılıkları atlamış olabilir ` +
      '(--no-optional, --omit=optional ya da başka bir platformda oluşturulmuş kilit dosyası).\n' +
      'Yeniden kur: npm install sezgi'
    );
  }
}

let ikili;
try {
  ikili = ikiliYolu();
  if (!fs.existsSync(ikili)) throw new Error(`Sezgi ikilisi bulunamadı: ${ikili}`);
} catch (e) {
  console.error(e.message);
  process.exit(1);
}

const sonuc = spawnSync(ikili, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
if (sonuc.error) {
  console.error(`Sezgi çalıştırılamadı (${path.basename(ikili)}): ${sonuc.error.message}`);
  process.exit(1);
}
if (sonuc.signal) process.kill(process.pid, sonuc.signal);
process.exit(sonuc.status === null ? 1 : sonuc.status);
