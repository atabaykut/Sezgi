// Benlik Kartı imzasını Sezgi'den BAĞIMSIZ olarak doğrular: RFC 8785 için referans `canonicalize`
// paketi, ML-DSA-65 (FIPS 204) için `@noble/post-quantum`. Rust ve JS aynı baytları imzalıyor mu?
// Kullanım: node imza-dogrula.mjs <benlik.json> <benlik-keys.json>   (paketler: kanit/acik/node_modules)
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const acik = path.join(path.dirname(fileURLToPath(import.meta.url)), 'acik', 'node_modules');
const cz = createRequire(path.join(acik, 'x.js'))('canonicalize');
const canonicalize = typeof cz === 'function' ? cz : cz.default || cz.canonicalize;
const { ml_dsa65 } = await import(pathToFileURL(path.join(acik, '@noble', 'post-quantum', 'ml-dsa.js')).href);

const kart = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const anahtarlar = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const { signature, ...govde } = kart;
if (signature.alg !== 'ML-DSA-65') {
  console.log(`✗ algoritma ${signature.alg}`);
  process.exit(1);
}
const anahtar = anahtarlar.keys.find(k => k.key_id === signature.key_id);
if (!anahtar) {
  console.log(`✗ '${signature.key_id}' anahtarı yok`);
  process.exit(1);
}
// SPEC 8: anahtarın algoritması imzanınkiyle aynı olmalı
if (anahtar.alg !== signature.alg) {
  console.log(`✗ anahtar belgesindeki algoritma ${anahtar.alg}, imzanınki ${signature.alg}`);
  process.exit(1);
}
// Katı base64 (RFC 4648, dolgulu): Buffer.from(...,'base64') geçersiz karakterleri sessizce atlar; Rust doğrulayıcısı
// reddeder. İki doğrulayıcı aynı kartta farklı karar vermesin.
const b64 = (s, ad) => {
  if (typeof s !== 'string' || s.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(s)) {
    console.log(`✗ ${ad} geçerli base64 değil`);
    process.exit(1);
  }
  return Buffer.from(s, 'base64');
};
const mesaj = new TextEncoder().encode(canonicalize(govde));
// FIPS 204 bağlam dizgisi (SPEC: "benlik-card")
const context = new TextEncoder().encode('benlik-card');
const ok = ml_dsa65.verify(b64(signature.value, 'imza'), mesaj, b64(anahtar.public_key, 'açık anahtar'), { context });
console.log(ok ? `✓ bağımsız doğrulama geçti (canonicalize + noble, ${mesaj.length} bayt)` : '✗ bağımsız doğrulama BAŞARISIZ');
process.exit(ok ? 0 : 1);
