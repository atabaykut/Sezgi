// Holdout giriş uyuşmazlıklarının elle hakemlenmesine yardımcı: her endpoint için metodun hemen üstündeki
// dekoratörleri ve sınıf dekoratörlerini okur, gerçek giriş gereksinimini kurala göre sınıflandırır.
// Kural (Ghostfolio kodu okunarak belirlendi, sonuca bakmadan önce HOLDOUT-ON-KAYIT.md'de "elle hakemlenecek" denmişti):
//   AuthGuard('jwt') / AuthGuard('api-key') ya da @RequiresScope(...)  -> gerekli (RequiresScope içinde AuthGuard('jwt') var)
//   @RequiresScopeOfAccess(...)                                          -> gerekli (erişim anahtarı kimlik yerine geçer)
//   AuthGuard('google' | 'oidc'), OAuthCallbackGuard                     -> herkese açık (girişi başlatan uç)
//   hiçbiri                                                               -> herkese açık
// Kullanım: node holdout-hakemle.js <uygulama kökü (apps/api)> <hakem.json> <sezgi kapilar.json>
'use strict';
const fs = require('fs');
const path = require('path');
const [kok, hakemYolu, sezgiYolu] = process.argv.slice(2);
const hakem = JSON.parse(fs.readFileSync(hakemYolu, 'utf8')).endpointler;
const sezgi = new Map(JSON.parse(fs.readFileSync(sezgiYolu, 'utf8')).endpointler.map(e => [e.id, e]));

function dekoratorler(dosya, fonksiyon) {
  const satirlar = fs.readFileSync(path.join(kok, dosya), 'utf8').split('\n');
  const tanim = new RegExp('^\\s*(public\\s+|private\\s+|async\\s+)*' + fonksiyon + '\\s*\\(');
  const i = satirlar.findIndex(l => tanim.test(l));
  // Metodun üstüne doğru: dekoratörler ve onların çok satırlı argümanları; önceki metodun kapanışında dur
  const ust = [];
  for (let j = i - 1; j >= 0; j--) {
    const l = satirlar[j];
    if (/^\s*}\s*$/.test(l)) break;
    ust.unshift(l);
  }
  const sinif = satirlar.slice(0, satirlar.findIndex(l => /export class/.test(l))).join(' ');
  return { metot: ust.join(' '), sinif };
}

function gercek(e) {
  const { metot, sinif } = dekoratorler(e.dosya, e.fonksiyon);
  const d = metot + ' ' + sinif;
  if (/AuthGuard\(\s*'(jwt|api-key)'\s*\)/.test(d) || /@RequiresScope\(/.test(d)) return ['gerekli', 'jwt / api-key / RequiresScope'];
  if (/@RequiresScopeOfAccess\(/.test(d)) return ['gerekli', 'RequiresScopeOfAccess (erişim anahtarı)'];
  if (/AuthGuard\(\s*'(google|oidc)'\s*\)|OAuthCallbackGuard/.test(d)) return ['herkese_acik', 'OAuth girişini başlatan uç'];
  return ['herkese_acik', 'giriş dekoratörü yok'];
}

let [sezgiDogru, hakemDogru] = [0, 0];
const sezgiYanlis = [];
for (const e of hakem) {
  const [dogru, neden] = gercek(e);
  const s = sezgi.get(e.id);
  if (e.giris === dogru) hakemDogru++;
  if (s && s.giris === dogru) sezgiDogru++;
  else sezgiYanlis.push(`${e.id}  sezgi=${s ? s.giris + '/' + s.giris_kaniti : 'yok'}  gerçek=${dogru} (${neden})`);
}
console.log(`Endpoint ${hakem.length} · Sezgi giriş doğru ${sezgiDogru} (%${((100 * sezgiDogru) / hakem.length).toFixed(1)}) · hakem giriş doğru ${hakemDogru}`);
const gruplar = {};
for (const y of sezgiYanlis) {
  const k = y.split('gerçek=')[1];
  (gruplar[k] = gruplar[k] || []).push(y.split('  ')[0]);
}
for (const [k, v] of Object.entries(gruplar)) console.log(`  Sezgi yanlış · ${v.length} · ${k}\n     ${v.join(', ')}`);
