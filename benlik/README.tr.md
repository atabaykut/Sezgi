# Benlik Kartı

**Bir uygulamanın, yapay zekâ ajanları için imzalı benlik beyanı.**

Benlik Kartı, uygulamanın kendi kodundan üretilen ve `/.well-known/benlik.json` adresinde yayınlanan tek bir JSON dosyasıdır. Bir yapay zekâ ajanına şunları söyler:

- Hangi işlemler var, her biri ne istiyor?
- Hangi kurallar bir işlemi reddeder, hangi rol gerekir, ne yan etkisi vardır?
- Yazma hangi gerçek kullanıcının kimliğiyle yapılır?
- Hangi veri, ne kadar süre tutulur?
- Son sürümden bu yana ne değişti?
- **Bunların her biri ne kadar kesin biliniyor, ne bilinmiyor?**

## Temel kurallar

- **Damgalar:** Her bilgi Kesin, Veri, Onaylı, Yorum ya da Bilinmiyor damgası taşır. Yalnızca Kesin damgalı işlem yazma yapabilir (Taslak 0.1.1; Veri ve Onaylı artık gözlem sayılır ve tek başına yazmaya yetmez).
- **Kimlik:** Etkin yetki = kullanıcının uygulamadaki yetkisi ∩ token izni. Bir rol ancak insan için doğrulanmışsa sayılır (insanın beyanı, imzası doğrulanmış kimlik belirteci ya da uygulamanın "ben" ucu); ajanın rol beyanı asla sayılmaz. İnsan yoksa yazma yoktur. Servis hesabıyla gerçek veriye yazılmaz.
- **İmza ve tazelik:** Kart imzalıdır. İmzası tutmayan kart yok sayılır. Kartla gelen anahtar bütünlüğü gösterir, kimliği değil: ajan, güvendiği (sabitlenmiş) bir anahtarla doğrulanmayan karta dayanarak yazmaz. Kartın geçerlilik süresi vardır; süresi dolan karta dayanarak yazılmaz. Bayatlık elle yazılmaz, kartın dayandığı dosyaların özetlerinden hesaplanır: dosya değiştiyse işlem bayattır ve yazma durur.
- **Kanıt:** Kanıt kayıtları kartın yanındaki ayrı pakettedir; kart her kaydın özetini ve Merkle kökünü taşır, imza böylece kanıtı da bağlar.
- **Kartta asla bulunmayanlar:** Token, kişisel veri, müşteri verisi, kaynak kodu.

Uluslararası biçimde alan adları İngilizcedir; Türkçe karşılıkları [SPEC.md](SPEC.md) ekinde.

## Lisans

Özet: [LICENSE](LICENSE) (metin CC BY 4.0; şema, örnekler, test vektörleri ve kod Apache-2.0).

Metin CC BY 4.0, şema ve örnekler Apache 2.0. Telif © 2026 Benlik Kartı geliştiricileri.

Sezgi, bu standardın referans üreticisidir.
