// Immich görev seti. Beklenen cevaplar Immich'in resmî OpenAPI dosyasından türetilir (biz yazmadık).
// Kullanım: node gorev-olustur.js <immich-openapi-specs.json>  → gorevler.json
'use strict';
const fs = require('fs');
const path = require('path');
const d = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const S = d.components.schemas;

const GOREVLER = [
  ['I01', 'Yeni bir albüm oluştur.', 'post', '/albums'],
  ['I02', 'Var olan bir albüme fotoğraf ekle.', 'put', '/albums/{id}/assets'],
  ['I03', 'Bir albümü başka kullanıcılarla paylaş.', 'put', '/albums/{id}/users'],
  ['I04', 'Fotoğrafları herkese açık bir bağlantıyla paylaşmak için paylaşım linki oluştur.', 'post', '/shared-links'],
  ['I05', 'Yüz tanımadaki iki kişiyi tek kişide birleştir.', 'post', '/people/merge'], // ilk sürümde yanlışlıkla kullanımdan kaldırılmış /people/{id}/merge seçilmişti
  ['I06', 'Yeni bir etiket oluştur.', 'post', '/tags'],
  ['I07', 'Yönetici olarak yeni bir kullanıcı hesabı oluştur.', 'post', '/admin/users'],
  ['I08', 'E-posta ve şifreyle giriş yap.', 'post', '/auth/login'],
  ['I09', 'Sunucunun ayakta olup olmadığını kontrol et.', 'get', '/server/ping'],
  ['I10', 'Bir albümdeki fotoğrafa beğeni ya da yorum ekle.', 'post', '/activities'],
  ['I11', 'Yeni bir API anahtarı oluştur.', 'post', '/api-keys'],
  ['I12', 'Yeni bir hatıra (memory) oluştur.', 'post', '/memories'],
  ['I13', 'Yönetici olarak yeni bir harici kütüphane tanımla.', 'post', '/libraries'],
  ['I14', 'Bir kullanıcıyı partner olarak ekleyip fotoğraflarını paylaşmaya başla.', 'post', '/partners'],
  ['I15', 'Yüz tanımadaki bir kişinin adını değiştir.', 'put', '/people/{id}'],
  ['I16', 'Birkaç fotoğrafı tek bir yığın (stack) altında grupla.', 'post', '/stacks'],
];

const gorevler = GOREVLER.map(([id, soru, m, p]) => {
  const o = d.paths[p] && d.paths[p][m];
  if (!o) throw new Error(`Resmî dokümanda yok: ${m.toUpperCase()} ${p}`);
  const ref = o.requestBody && o.requestBody.content && Object.values(o.requestBody.content)[0].schema;
  const sema = ref && ref.$ref ? S[ref.$ref.split('/').pop()] : null;
  const roller = [o['x-immich-permission'], o['x-immich-admin-only'] ? 'admin' : null].filter(Boolean).map(r => r.toUpperCase()).sort();
  return {
    id, tur: 'cagri', soru,
    beklenen: {
      yontem: m.toUpperCase(),
      yol: '/api' + p.replace(/\{[^}]+\}/g, '{}'),
      giris: (o.security || []).length ? 'gerekli' : 'herkese_acik',
      roller,
      zorunlu_alanlar: sema && sema.required ? [...sema.required].sort() : [],
    },
    kaynak: `immich-openapi-specs.json ${m.toUpperCase()} ${p}`,
  };
});
fs.writeFileSync(path.join(__dirname, 'gorevler.json'), JSON.stringify({
  uygulama: 'Immich server (commit aec0dd1)',
  hakem: 'Immich resmî OpenAPI dosyası (open-api/immich-openapi-specs.json)',
  roller_aciklama: "gereken izinler (ör. album.create) ve yönetici gerekiyorsa 'admin'; yoksa boş",
  gorev_sayisi: gorevler.length,
  gorevler,
}, null, 2));
for (const g of gorevler) console.log(g.id, g.beklenen.yontem, g.beklenen.yol, `[${g.beklenen.giris}]`, g.beklenen.roller.join(','), `{${g.beklenen.zorunlu_alanlar.join(',')}}`);
