"use strict";
// Sezgi Ayna. Haritadan gelen her değer güvenilmezdir (depoya işlenmiş bir .sezgi/ klasöründen gelebilir):
// sayfaya yalnızca DOM API ve metin düğümü (textContent) ile yazılır, HTML metni hiç üretilmez.
// Türü beklenenden farklı değerler (ör. sayı yerine metin) de yalnızca metin olarak gösterilir.

const $ = s => document.querySelector(s);
const dizi = x => (Array.isArray(x) ? x : []);
const nesne = x => (x && typeof x === "object" && !Array.isArray(x) ? x : {});
const metin = x => (x == null ? "" : typeof x === "object" ? JSON.stringify(x) : String(x));

// Ayna oturum anahtarı: Ayna'yı başlatanın gördüğü adresteki #anahtar=… (parça sunucuya gönderilmez). Token üretme ve
// iptal istekleri bunu başlıkta taşır; Origin tek başına yetmez. Sayfa yenilenince de kalsın diye sekme deposunda tutulur
// ve adres satırından silinir.
// Adres parçası `#anahtar=<anahtar>` ya da `#anahtar=<anahtar>&<seçilecek kapı>` olabilir.
let aynaAnahtari = "";
try { aynaAnahtari = sessionStorage.getItem("ayna-anahtar") || ""; } catch (_) {}
if (location.hash.startsWith("#anahtar=")) {
  const [a, ...kalan] = location.hash.slice("#anahtar=".length).split("&");
  aynaAnahtari = a.replace(/[^0-9a-f]/gi, "");
  try { sessionStorage.setItem("ayna-anahtar", aynaAnahtari); } catch (_) {}
  history.replaceState(null, "", location.pathname + location.search + (kalan.length ? "#" + kalan.join("&") : ""));
}

// Dil: sayfanın dili sunucudan gelir (<html lang>: `sezgi ayna --lang`, SEZGI_LANG ya da sistem dili; adreste ?lang=).
// Kullanıcının seçimi bu tarayıcıda saklanır; sunucunun verdiği metinler (kart, soru, ayrıntı) aynı dilde istenir.
const DIL = document.documentElement.lang === "en" ? "en" : "tr";
let yonlendiriliyor = false;
try {
  const tercih = localStorage.getItem("ayna-dil");
  if ((tercih === "tr" || tercih === "en") && tercih !== DIL && !new URLSearchParams(location.search).has("lang")) {
    yonlendiriliyor = true;
    location.replace(location.pathname + "?lang=" + tercih + location.hash);
  }
} catch (_) {}
const dilli = u => u + (u.includes("?") ? "&" : "?") + "lang=" + DIL;
const yazmaBasliklari = () => ({ "Content-Type": "application/json", "X-Sezgi-Anahtar": aynaAnahtari });
// Bütün veri istekleri anahtarı taşır (Ayna anahtarsız veri vermez)
const api = (u, s = {}) => fetch(dilli(u), { ...s, headers: { ...(s.headers || {}), "X-Sezgi-Anahtar": aynaAnahtari } });

/** Öğe üretir: h("span", { class: "rozet" }, "metin", altOge). Öznitelik değerleri ve çocuklar metin olarak yazılır. */
function h(etiket, oz, ...cocuklar) {
  const e = document.createElement(etiket);
  for (const [k, v] of Object.entries(oz || {})) {
    if (v == null || v === false) continue;
    if (k === "class") e.className = metin(v);
    else if (k.startsWith("on") && typeof v === "function") e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, metin(v));
  }
  ekle(e, cocuklar);
  return e;
}
function ekle(e, cocuklar) {
  for (const c of cocuklar.flat(Infinity)) {
    if (c == null || c === false) continue;
    e.append(c instanceof Node ? c : document.createTextNode(metin(c)));
  }
}
function yerlestir(hedef, ...cocuklar) {
  hedef.replaceChildren();
  ekle(hedef, cocuklar);
}

// Ekrandaki metinler tek yerde (M): Türkçe asıl, İngilizcesi M_EN (aynı anahtarlar). Durum ve güven yalnızca renkle
// ayrılmaz: her rozet simge ve metin taşır. Sunucudan gelen metinler (kart, soru, ayrıntı) sunucuda çevrilir.
const M = {
  anahtarsiz: "Bu sayfa Ayna oturum anahtarı olmadan (ya da eski bir anahtarla) açıldı. Ayna'yı başlattığınız terminaldeki adresle (…/#anahtar=…) açın; adresi yeniden görmek için: sezgi ayna --adres",
  giris: { gerekli: "giriş gerekli", herkese_acik: "herkese açık", istege_bagli: "giriş isteğe bağlı" },
  onermeAdi: { GIRIS: "giriş", ROLLER: "roller", GOVDE: "gövde", SORGU: "sorgu", VERI: "veri", VAR: "tanım" },
  durum: { hazir: "✓ hazır", kurulmamis: "○ kurulmamış", uygulanamaz: "– uygulanamaz", bilinmiyor: "? bilinmiyor" },
  altDurumSimge: { hazir: "✓", kurulmamis: "○", uygulanamaz: "–", bilinmiyor: "?" },
  guven: { certain: "kesin", observed: "gözlem", inferred: "yorum", unknown: "bilinmiyor" },
  guvenBaslik: "güven: ",
  yanit: { yanitli: "yanıtlı", kismen: "kısmen", bilinmiyor: "yanıt yok" },
  risk: { okuma: "okuma", yazma: "yazma", silme: "silme" },
  geri: { evet: "↺ geri alınabilir", kismen: "↺ kısmen geri alınabilir", hayir: "⚠ geri alınamaz", bilinmiyor: "? geri alma bilinmiyor", okuma: "okuma yöntemi" },
  onay: { gerekmez: "✓ onay gerekmez", kosullu: "◐ koşullu onaysız", her_seferinde: "✋ insan onayı ister" },
  // Kapılar
  kapiSayisi: (n, top) => `${n} / ${top} kapı`,
  ozetYok: "Kodda özet yok; fonksiyon adından okundu",
  ozetYokRozet: "yorum: özet yok, addan",
  kalkmisEk: " · kalkmış",
  ikincilEk: " · ikincil",
  eslesenKapiYok: "Eşleşen kapı yok.",
  zorunluBelirsiz: "zorunluluğu belirsiz",
  zorunlu: "zorunlu",
  istegeBagli: "isteğe bağlı",
  tamami: "(tamamı)",
  semaCozulemedi: "şema çözülemedi",
  semaCozulemediNot: "Bu kapıya yazma önerilmez; alanlar bilinmiyor.",
  degerler: "değerler: ",
  alan: "Alan",
  tip: "Tip",
  kurallar: "Kurallar",
  kanitBaslik: "Kanıt önermeleri ve güvenleri",
  eskiDosya: "eski (dosya değişti)",
  listeAcik: " · liste açık",
  kanitNot: "Kesin: koddan tek anlamlı. Yorum: çıkarım. Bilinmiyor: kanıt yok; yokluk demek değildir.",
  kapi: "Kapı",
  govde: "Gövde",
  govdeJson: "Gövde (JSON)",
  sorgu: "Sorgu",
  sorguJson: "Sorgu (JSON)",
  girisCikarim: "giriş kararı çıkarım",
  rol: "rol: ",
  surum: "sürüm ",
  kalkmis: "kullanımdan kalkmış",
  ikincil: "ikincil uygulama: ",
  korumalar: "korumalar: ",
  girdiler: "Girdiler",
  yolParametreleri: "Yol parametreleri",
  girdiYok: "Bu işlemin girdisi yok.",
  onizle: "Önizle",
  onizleNot: "İsteği kurallara karşı sınar. Uygulama çağrılmaz, hiçbir şey gönderilmez.",
  girisYapmis: " Giriş yapmış kullanıcı",
  roller: "Roller",
  rollerIpucu: "Roller (virgülle), boş: kontrol etme",
  gecersizJson: (ad, e) => `${ad} geçerli JSON değil: ${e}`,
  // Kapsam
  girdiliKapi: (k, a, r) => `${k} girdili kapı · ${a} alan · ${r} kural`,
  belirsizAlan: n => `${n} alanın zorunluluğu belirsiz`,
  belirsizAlanNot: " Doğrulayıcısı yok ya da özel dekoratörün tanımı izlenemedi.",
  cozulemeyenSema: n => `${n} şema çözülemedi`,
  cozulemeyenSemaNot: " Bu kapılara yazma önerilmez.",
  butunSemalar: "bütün şemalar çözüldü",
  denetlenemeyenKurallar: "Önizlemede kontrol edilemeyen kurallar (özel doğrulayıcı ya da sabit adlı sınır): ",
  veriBaglari: "Veri bağları",
  bagliKapi: (b, top) => `${b} / ${top} kapı bir tabloya bağlı · `,
  sema: "şema ",
  tabloSorgudan: "tablo adları sorgulardan (şema dosyası yok; yorum)",
  bagsizKapi: n => `${n} kapıda tablo bağı yok`,
  veriYok: "Veritabanı tanımı ve tablo bağı bulunamadı (desteklenen: Prisma, SQLModel / SQLAlchemy, Express sorguları).",
  guvenBolum: "Güven",
  onermeSayisi: n => `${n} önerme · `,
  kanitRev: " · kanıt deposu rev ",
  eskiOnerme: n => `${n} önerme eski`,
  eskiOnermeNot: " Dayandığı dosya haritadan sonra değişti: sezgi init.",
  celiski: n => `${n} önermede çelişki`,
  celiskiNot: " Güven en fazla yorum.",
  kapilar: "Kapılar",
  anaKapi: n => `${n} kapı ana uygulamada · `,
  rolKisitli: n => ` · ${n} rol kısıtlı`,
  ikincilUygulama: "İkincil uygulama ",
  ikincilKapi: n => `: ${n} kapı (Benlik Kartı'na girmez)`,
  kalkmisKapi: n => `${n} kapı kullanımdan kaldırılmış:`,
  okunamayanRota: n => `kapsam kısmen · okunamayan rota kaynağı: ${n} yer`,
  okunamayanRotaNot: " Kod bu yerlerde rota kaydediyor ama okunamadı; bu işlemler haritada yok, kapı sayısı kesin değil.",
  okuma: "Okuma",
  dosyaSatir: (d, s) => `${d} dosya · ${s} satır`,
  kardes: " · kardeş paketler: ",
  kismenOkunan: n => `${n} dosya kısmen okundu`,
  kismenOkunanNot: " Bu dosyalardaki bilgi eksik olabilir.",
  tamOkundu: "bütün dosyalar tam okundu",
  temizRapor: "Temiz bir rapor \"bilinen eksik yok\" demektir, eksiksizlik kanıtı değildir.",
  // İmza
  imza: {
    guvenilir: (alg, id) => `imza güvenilir · sabitlenmiş anahtar · ${alg} · ${id}`,
    uyusmuyor: id => `imza sabitlenmiş anahtarla UYUŞMUYOR · ${id}`,
    degisti: id => `imza tutarlı ama anahtar DEĞİŞTİ · ${id}`,
    tutarli: (alg, id) => `imza kendi içinde tutarlı · anahtar doğrulanmadı · ${alg} · ${id}`,
    gecersiz: "imza GEÇERSİZ: kart değişmiş",
    anahtarYok: "imzalı · açık anahtar yok, doğrulanamadı",
    imzasiz: "imzasız",
    degistiBaslik: "Bu uygulama için daha önce başka bir anahtar görüldü. Yeni anahtarı uygulama sahibinden doğrulayın; doğruysa: sezgi anahtar sabitle <benlik.json> <benlik-keys.json>",
    tutarliBaslik: "Kart, yanındaki benlik-keys.json ile tutarlı. Bu yalnızca kartın o anahtarla imzalandığını gösterir; anahtarın uygulama sahibine ait olduğunu göstermez (kartı ve anahtarı birlikte getiren biri de tutarlı imza üretebilir). Sahibin yayınladığı /.well-known/benlik-keys.json ile karşılaştırın.",
  },
  // Sezgi sekmesi
  anlamBaslik: "Bu uygulama",
  alanBaslik: "İş alanı",
  alanYok: "İş alanı belirlenemedi",
  amac: "Neden var (kendi beyanı)",
  amacYok: "README'de ya da paket tanımında uygulamayı anlatan bir cümle yok; Sezgi amacı uydurmaz.",
  problem: "Çözdüğü iş",
  kullanicilar: "Kullanıcı tipleri",
  yetenekler: n => `${n} iş yeteneği`,
  islemSayisi: n => `${n} işlem`,
  anlamNot: "Anlam çıkarımı en fazla yorumdur; alıntı uygulamanın kendi beyanıdır, doğrulanmış sayılmaz.",
  araniyor: "Aranıyor…",
  sonucYok: q => `“${q}” için eşleşen işlem bulunamadı. Uygulamanın kendi kelimeleriyle deneyin (ör. modül ya da tablo adı).`,
  sonucSayisi: (n, top) => (n > top ? `${n} eşleşme; ilk ${top} tanesi:` : `${n} eşleşme:`),
  plan: "Görev planı (öneri; Sezgi planı yürütmez)",
  ayrinti: "Ayrıntı",
  sonraki: "Sıradaki adım",
  kurulumTamam: "✓ kurulum tamam",
  kurulumTamamMetin: " Yapay zekâ aracınızdan Sezgi'ye sorabilirsiniz; bu uygulamanın bilinenleri \"Ne biliyoruz\" sekmesinde.",
  klasor: "klasör ",
  kaynaksiz: "kaynaksız",
  islemEk: " işlem",
  degisen: n => `! ${n} dosya haritadan sonra değişti`,
  haritaGuncel: "✓ harita güncel",
  haritaYok: "○ harita yok",
  cati: "çatı: ",
  catiYok: "desteklenen çatı bulunamadı",
  // Ne biliyoruz
  yardimci: "Yardımcı olabilir: ",
  bilinmeyen: "Bilinmeyen: ",
  bilinmiyor: "Bilinmiyor: ",
  soru: n => `${n} soru`,
  yanitliSayi: n => `✓ ${n} yanıtlı`,
  kismenSayi: n => `◐ ${n} kısmen`,
  bilinmiyorSayi: n => `? ${n} bilinmiyor`,
  islemYukleniyor: "İşlemin etkisi yükleniyor…",
  islemAlinamadi: "İşlemin ayrıntısı alınamadı.",
  // İzinler ve token
  izinRisk: { oku: "okuma", yaz: "yazma", yikici: "yıkıcı · onaylı" },
  mod: {
    gozlem: "Mod: gözlem. Token'la bağlanan yapay zekâ seçtiğin işlemleri görür ve önizler; uygulamaya istek gitmez. İstek iletmek için: sezgi mod okuma (ya da yazma: test kopyasında ya da insanın terminalde açtığı gerçek veride).",
    okuma: "Mod: okuma. Seçtiğin okuma işlemleri uygulamaya iletilebilir; yazmalar yalnızca önizlenir.",
    yazma: "Mod: yazma (test kopyası). Seçtiğin yazmalar insanın kendi oturumuyla iletilir; yıkıcı işlemler her seferinde insan onayı bekler.",
  },
  tokenDeposu: e => `Token deposu açılamadı: ${e}`,
  seciliIslem: n => `${n} işlem seçili`,
  tokenUret: "Token üret",
  tokenUretN: n => `Token üret (${n} işlem)`,
  kalkmisKisa: "kalkmış",
  okur: " · okur: ",
  yazar: " · yazar: ",
  rolListe: " · rol: ",
  adsiz: "(adsız)",
  modulIslem: n => `${n} işlem`,
  modulSecili: n => ` · ${n} seçili`,
  eslesenIslemYok: "Eşleşen işlem yok.",
  kopyala: "Kopyala",
  kopyalandi: "Kopyalandı",
  kopyalanamadi: "Kopyalanamadı: elle seçin",
  tokenAdi: "Token'a bir ad ver (kimin ya da hangi araç için olduğu).",
  tokenUretildi: "Token üretildi",
  birDaha: "Bir daha gösterilmez;",
  simdiKopyala: " şimdi kopyala. Sezgi yalnızca özetini saklar.",
  araclar: "Yapay zekânın göreceği tool'lar",
  araclarNot: n => `Seçtiğin ${n} işlem kendi tool'u olarak verilir (genel tool'lar kimsin, ara, tani, onizle, cagir da durur). Her çağrı aynı kapılardan geçer.`,
  esikNot: (n, e) => `Seçilen ${n} işlem eşiği (${e}) aştığı için işlemler tek tek tool olarak verilmez; yapay zekâ ara → tani → cagir ile çalışır.`,
  bagla: "Yapay zekâya bağla",
  tokenDurum: { gecerli: "geçerli", iptal: "iptal", suresi_dolmus: "süresi dolmuş" },
  iptalSoru: ad => `'${ad}' iptal edilsin mi? Bu token'la gelen istekler hemen reddedilir.`,
  iptalEt: "İptal et",
  tokenOzet: (n, onay) => `${n} izin · onay ${onay}`,
  bitis: t => ` · bitiş ${t}`,
  izinler: "İzinler",
  tokenYok: "Henüz token yok.",
  // Üst bilgi ve sayılar
  haritaYokBaslik: "· harita yok",
  kod: ref => `kod ${ref}`,
  kodGitYok: "kod sürümü yok (git yok)",
  kodOkunamadi: "kod sürümü okunamadı (git commit'i çözülemedi)",
  kodBilinmiyor: "kod sürümü bilinmiyor",
  sayilar: { kapi: "kapı", giris: "giriş gerekli", alan: "girdi alanı", kural: "doğrulama kuralı", bagli: "tabloya bağlı kapı", bagYok: "veri bağı yok", sema: "çözülemeyen şema", sure: "harita süresi" },
  kartIslem: " işlem",
  kartTam: " tam",
  benlikYok: "benlik.json yok",
};

// İngilizcede sayıya uyan ad: 1 için tekil, başka sayılar için çoğul ("1 claim", "2 claims")
const cogul = (n, tekil, cok) => (Number(n) === 1 ? tekil : cok);
const sayili = (n, tekil, cok) => `${n} ${cogul(n, tekil, cok)}`;

const M_EN = {
  anahtarsiz: "This page was opened without the Ayna session key (or with an old one). Open it with the address printed in the terminal where you started Ayna (…/#anahtar=…); to see the address again: sezgi ayna --adres",
  giris: { gerekli: "login required", herkese_acik: "public", istege_bagli: "login optional" },
  onermeAdi: { GIRIS: "login", ROLLER: "roles", GOVDE: "body", SORGU: "query", VERI: "data", VAR: "definition" },
  durum: { hazir: "✓ ready", kurulmamis: "○ not set up", uygulanamaz: "– not applicable", bilinmiyor: "? unknown" },
  altDurumSimge: { hazir: "✓", kurulmamis: "○", uygulanamaz: "–", bilinmiyor: "?" },
  guven: { certain: "certain", observed: "observed", inferred: "inferred", unknown: "unknown" },
  guvenBaslik: "trust: ",
  yanit: { yanitli: "answered", kismen: "partly", bilinmiyor: "no answer" },
  risk: { okuma: "read", yazma: "write", silme: "delete" },
  geri: { evet: "↺ can be undone", kismen: "↺ can be partly undone", hayir: "⚠ cannot be undone", bilinmiyor: "? undo unknown", okuma: "read method" },
  onay: { gerekmez: "✓ no approval needed", kosullu: "◐ conditionally without approval", her_seferinde: "✋ needs human approval" },
  kapiSayisi: (n, top) => `${n} / ${sayili(top, "endpoint", "endpoints")}`,
  ozetYok: "No summary in the code; read from the function name",
  ozetYokRozet: "inferred: no summary, from the name",
  kalkmisEk: " · deprecated",
  ikincilEk: " · secondary",
  eslesenKapiYok: "No matching endpoint.",
  zorunluBelirsiz: "required: uncertain",
  zorunlu: "required",
  istegeBagli: "optional",
  tamami: "(whole)",
  semaCozulemedi: "schema not resolved",
  semaCozulemediNot: "Writing to this endpoint is not advised; its fields are unknown.",
  degerler: "values: ",
  alan: "Field",
  tip: "Type",
  kurallar: "Rules",
  kanitBaslik: "Evidence claims and their trust",
  eskiDosya: "stale (file changed)",
  listeAcik: " · open list",
  kanitNot: "Certain: unambiguous in the code. Inferred: a deduction. Unknown: no evidence; it does not mean absent.",
  kapi: "Endpoint",
  govde: "Body",
  govdeJson: "Body (JSON)",
  sorgu: "Query",
  sorguJson: "Query (JSON)",
  girisCikarim: "login decision inferred",
  rol: "role: ",
  surum: "version ",
  kalkmis: "deprecated",
  ikincil: "secondary app: ",
  korumalar: "guards: ",
  girdiler: "Inputs",
  yolParametreleri: "Path parameters",
  girdiYok: "This operation takes no input.",
  onizle: "Preview",
  onizleNot: "Checks the request against the rules. The application is not called; nothing is sent.",
  girisYapmis: " Logged-in user",
  roller: "Roles",
  rollerIpucu: "Roles (comma-separated), empty: do not check",
  gecersizJson: (ad, e) => `${ad} is not valid JSON: ${e}`,
  girdiliKapi: (k, a, r) => `${sayili(k, "endpoint", "endpoints")} with input · ${sayili(a, "field", "fields")} · ${sayili(r, "rule", "rules")}`,
  belirsizAlan: n => `${sayili(n, "field", "fields")} with uncertain required flag`,
  belirsizAlanNot: " No validator, or the custom decorator's definition could not be traced.",
  cozulemeyenSema: n => `${sayili(n, "schema", "schemas")} not resolved`,
  cozulemeyenSemaNot: " Writing to these endpoints is not advised.",
  butunSemalar: "all schemas resolved",
  denetlenemeyenKurallar: "Rules the preview cannot check (custom validator or a bound given by a named constant): ",
  veriBaglari: "Data links",
  bagliKapi: (b, top) => `${b} / ${sayili(top, "endpoint", "endpoints")} linked to a table · `,
  sema: "schema ",
  tabloSorgudan: "table names from queries (no schema file; inferred)",
  bagsizKapi: n => `${sayili(n, "endpoint", "endpoints")} without a table link`,
  veriYok: "No database definition or table link found (supported: Prisma, SQLModel / SQLAlchemy, Express queries).",
  guvenBolum: "Trust",
  onermeSayisi: n => `${sayili(n, "claim", "claims")} · `,
  kanitRev: " · evidence store rev ",
  eskiOnerme: n => `${sayili(n, "claim", "claims")} stale`,
  eskiOnermeNot: " The file they rest on changed after the map: sezgi init.",
  celiski: n => `${sayili(n, "claim", "claims")} contradicted`,
  celiskiNot: " Trust is at most inferred.",
  kapilar: "Endpoints",
  anaKapi: n => `${sayili(n, "endpoint", "endpoints")} in the main app · `,
  rolKisitli: n => ` · ${n} role-restricted`,
  ikincilUygulama: "Secondary app ",
  ikincilKapi: n => `: ${sayili(n, "endpoint", "endpoints")} (not in the Benlik Card)`,
  kalkmisKapi: n => `${sayili(n, "endpoint", "endpoints")} deprecated:`,
  okunamayanRota: n => `coverage partial · unreadable route source: ${sayili(n, "place", "places")}`,
  okunamayanRotaNot: " The code registers routes here but they could not be read; these operations are not in the map and the endpoint count is not certain.",
  okuma: "Reading",
  dosyaSatir: (d, s) => `${sayili(d, "file", "files")} · ${sayili(s, "line", "lines")}`,
  kardes: " · sibling packages: ",
  kismenOkunan: n => `${sayili(n, "file", "files")} partly read`,
  kismenOkunanNot: " What these files hold may be incomplete.",
  tamOkundu: "all files read in full",
  temizRapor: "A clean report means \"no known gaps\"; it is not proof of completeness.",
  imza: {
    guvenilir: (alg, id) => `signature trusted · pinned key · ${alg} · ${id}`,
    uyusmuyor: id => `signature does NOT MATCH the pinned key · ${id}`,
    degisti: id => `signature consistent but the key CHANGED · ${id}`,
    tutarli: (alg, id) => `signature self-consistent · key not verified · ${alg} · ${id}`,
    gecersiz: "signature INVALID: the card was changed",
    anahtarYok: "signed · no public key, not verified",
    imzasiz: "unsigned",
    degistiBaslik: "A different key was seen for this app before. Verify the new key with the app's owner; if it is right: sezgi anahtar sabitle <benlik.json> <benlik-keys.json>",
    tutarliBaslik: "The card matches the benlik-keys.json next to it. This only shows that the card was signed with that key, not that the key belongs to the app's owner (whoever brings the card and the key together can also produce a consistent signature). Compare it with the /.well-known/benlik-keys.json the owner publishes.",
  },
  anlamBaslik: "This application",
  alanBaslik: "Business domain",
  alanYok: "Business domain could not be determined",
  amac: "Why it exists (its own words)",
  amacYok: "No sentence in the README or the package description describes the app; Sezgi does not invent a purpose.",
  problem: "The work it does",
  kullanicilar: "User types",
  yetenekler: n => `${n} business ${cogul(n, "capability", "capabilities")}`,
  islemSayisi: n => sayili(n, "operation", "operations"),
  anlamNot: "Meaning is at most inferred; the quote is the app's own statement and is not treated as verified.",
  araniyor: "Searching…",
  sonucYok: q => `No operation matches “${q}”. Try the app's own words (e.g. a module or table name).`,
  sonucSayisi: (n, top) => (n > top ? `${n} matches; the first ${top}:` : `${sayili(n, "match", "matches")}:`),
  plan: "Task plan (a suggestion; Sezgi does not run it)",
  ayrinti: "Details",
  sonraki: "Next step",
  kurulumTamam: "✓ setup complete",
  kurulumTamamMetin: " You can ask Sezgi from your AI tool; what is known about this app is on the \"What we know\" tab.",
  klasor: "folder ",
  kaynaksiz: "sourceless",
  islemEk: " operations",
  degisen: n => `! ${sayili(n, "file", "files")} changed after the map`,
  haritaGuncel: "✓ map up to date",
  haritaYok: "○ no map",
  cati: "framework: ",
  catiYok: "no supported framework found",
  yardimci: "May help: ",
  bilinmeyen: "Unknown: ",
  bilinmiyor: "Unknown: ",
  soru: n => sayili(n, "question", "questions"),
  yanitliSayi: n => `✓ ${n} answered`,
  kismenSayi: n => `◐ ${n} partly`,
  bilinmiyorSayi: n => `? ${n} unknown`,
  islemYukleniyor: "Loading the operation's effects…",
  islemAlinamadi: "The operation's details could not be loaded.",
  izinRisk: { oku: "read", yaz: "write", yikici: "destructive · approved" },
  mod: {
    gozlem: "Mode: observe. An AI connected with the token sees and previews the operations you choose; no request reaches the app. To forward requests: sezgi mod okuma (or yazma: on a test copy or on real data a human opened in a terminal).",
    okuma: "Mode: read. The read operations you choose can be forwarded to the app; writes are only previewed.",
    yazma: "Mode: write (test copy). The writes you choose are forwarded with the human's own session; destructive operations wait for human approval every time.",
  },
  tokenDeposu: e => `The token store could not be opened: ${e}`,
  seciliIslem: n => `${sayili(n, "operation", "operations")} selected`,
  tokenUret: "Create token",
  tokenUretN: n => `Create token (${sayili(n, "operation", "operations")})`,
  kalkmisKisa: "deprecated",
  okur: " · reads: ",
  yazar: " · writes: ",
  rolListe: " · role: ",
  adsiz: "(unnamed)",
  modulIslem: n => sayili(n, "operation", "operations"),
  modulSecili: n => ` · ${n} selected`,
  eslesenIslemYok: "No matching operation.",
  kopyala: "Copy",
  kopyalandi: "Copied",
  kopyalanamadi: "Could not copy: select it by hand",
  tokenAdi: "Give the token a name (who or which tool it is for).",
  tokenUretildi: "Token created",
  birDaha: "It will not be shown again;",
  simdiKopyala: " copy it now. Sezgi keeps only its digest.",
  araclar: "Tools the AI will see",
  araclarNot: n => `The ${sayili(n, "operation", "operations")} you chose ${cogul(n, "is given as its own tool", "are given as their own tools")} (the general tools app, search, describe, preview and call stay too). Every call passes the same gates.`,
  esikNot: (n, e) => `The ${n} chosen operations exceed the threshold (${e}), so they are not given as separate tools; the AI works with search → describe → call.`,
  bagla: "Connect the AI",
  tokenDurum: { gecerli: "valid", iptal: "revoked", suresi_dolmus: "expired" },
  iptalSoru: ad => `Revoke '${ad}'? Requests with this token will be rejected at once.`,
  iptalEt: "Revoke",
  tokenOzet: (n, onay) => `${sayili(n, "permission", "permissions")} · approval ${onay}`,
  bitis: t => ` · expires ${t}`,
  izinler: "Permissions",
  tokenYok: "No tokens yet.",
  haritaYokBaslik: "· no map",
  kod: ref => `code ${ref}`,
  kodGitYok: "no code version (no git)",
  kodOkunamadi: "code version could not be read (git commit not resolved)",
  kodBilinmiyor: "code version unknown",
  sayilar: { kapi: "endpoints", giris: "login required", alan: "input fields", kural: "validation rules", bagli: "endpoints linked to a table", bagYok: "no data links", sema: "unresolved schemas", sure: "map time" },
  kartIslem: " operations",
  kartTam: " complete",
  benlikYok: "no benlik.json",
};
if (DIL === "en") Object.assign(M, M_EN);

const GUVEN_SINIFI = { certain: "rozet iyi", observed: "rozet vurgu", inferred: "rozet uyari", unknown: "rozet kotu", eski: "rozet kotu" };
const YONTEMLER = ["GET", "POST", "PUT", "PATCH", "DELETE"];
let harita, kart, rapor, imza, kanit, kapilar = [], secili = null;

try { const t = localStorage.getItem("ayna-tema"); if (t === "light" || t === "dark") document.documentElement.dataset.theme = t; } catch (_) {}
$("#tema").addEventListener("click", () => {
  const koyu = document.documentElement.dataset.theme ? document.documentElement.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  const yeni = koyu ? "light" : "dark";
  document.documentElement.dataset.theme = yeni;
  try { localStorage.setItem("ayna-tema", yeni); } catch (_) {}
});
// Dil düğmesi: öteki dile geçer; seçim bu tarayıcıda saklanır, sayfa o dilde yeniden açılır (anahtar sekme deposunda kalır)
$("#dil").addEventListener("click", () => {
  const yeni = DIL === "en" ? "tr" : "en";
  try { localStorage.setItem("ayna-dil", yeni); } catch (_) {}
  location.replace(location.pathname + "?lang=" + yeni + location.hash);
});

// Sekmeler (WAI-ARIA sekme deseni): seçili sekme Tab ile odaklanır, sekmeler arasında ok tuşları, Home ve End
const sekmeler = () => [...document.querySelectorAll('nav [role="tab"]')].filter(b => !b.hidden);
function sekmeAc(ad, odakla = false) {
  // Seçili sekme gövdede: dar ekranda ölçüm kutuları tanıtımın önüne geçmesin diye (ayna.css)
  document.body.dataset.sekme = ad;
  for (const b of document.querySelectorAll('nav [role="tab"]')) {
    const secili = b.dataset.sekme === ad;
    b.setAttribute("aria-selected", secili);
    b.tabIndex = secili ? 0 : -1;
    $("#" + b.dataset.sekme).hidden = !secili;
    if (secili && odakla) b.focus();
  }
  if (ad === "izinler" && !izin.yuklendi) izinYukle();
}
for (const b of document.querySelectorAll('nav [role="tab"]')) b.addEventListener("click", () => sekmeAc(b.dataset.sekme));
$("nav").addEventListener("keydown", e => {
  const xs = sekmeler(), i = xs.indexOf(document.activeElement);
  if (i < 0) return;
  const j = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: xs.length - 1 }[e.key];
  if (j === undefined) return;
  e.preventDefault();
  sekmeAc(xs[(j + xs.length) % xs.length].dataset.sekme, true);
});

/** Özeti olmayan kapı: fonksiyon adından okunur metin (deleteDatabaseBackup -> "delete database backup"), yorum sayılır */
const addanOku = ad => metin(ad).replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/[_\-.]+/g, " ").toLowerCase().trim();

const veri = k => nesne(k.veri);
const girdiler = k => [...dizi(k.govde), ...dizi(k.sorgu)];
const semaSorunu = k => girdiler(k).some(g => nesne(g).sema && nesne(nesne(g).sema).bulunamadi === true);
const alanAdlari = k => girdiler(k).flatMap(g => Object.keys(nesne(nesne(nesne(g).sema).alanlar)));
const yontemSinifi = y => (YONTEMLER.includes(y) ? "yontem " + y : "yontem");

// Sunucu araması: Türkçe ya da İngilizce istek ("kullanıcı sil") kodun kelimelerine ve HTTP yöntemine çevrilir; sonuç
// yerel metin eşleşmesine eklenir ve sunucunun sırası öne alınır
let sunucuSonucu = { q: "", ids: [] };
let araZamanlayici = null;
function sunucuAra() {
  const q = $("#ara").value.trim();
  clearTimeout(araZamanlayici);
  if (!q) { sunucuSonucu = { q: "", ids: [] }; return; }
  araZamanlayici = setTimeout(async () => {
    try {
      const c = await api("/api/ara", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ sorgu: q }) });
      const v = nesne(await c.json());
      if ($("#ara").value.trim() === q) { sunucuSonucu = { q, ids: dizi(v.kapilar).map(metin) }; listele(); }
    } catch (_) { /* arama sunucusu yoksa yerel eşleşme yeter */ }
  }, 200);
}

function listele() {
  const q = $("#ara").value.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const sunucudan = sunucuSonucu.q && sunucuSonucu.q === $("#ara").value.trim() ? sunucuSonucu.ids : [];
  const y = $("#s-yontem").value, g = $("#s-giris").value, d = $("#s-diger").value;
  const sonuc = kapilar.filter(k => {
    if (y && k.yontem !== y) return false;
    if (g && k.giris !== g) return false;
    if (d === "yazar" && !dizi(veri(k).yazar).length) return false;
    if (d === "bilinmeyen" && !semaSorunu(k)) return false;
    if (d === "kalkmis" && k.kullanimdan_kalkmis !== true) return false;
    if (d === "ikincil" && k.ana !== false) return false;
    if (!q.length) return true;
    const aranan = [k.id, k.kontrolcu, k.fonksiyon, k.ozet, ...alanAdlari(k), ...dizi(veri(k).okur), ...dizi(veri(k).yazar), ...dizi(k.roller)].map(metin).join(" ").toLowerCase();
    return q.every(w => aranan.includes(w)) || sunucudan.includes(metin(k.id));
  });
  if (sunucudan.length) {
    const sira = id => { const i = sunucudan.indexOf(metin(id)); return i < 0 ? sunucudan.length : i; };
    sonuc.sort((a, b) => sira(a.id) - sira(b.id));
  }
  $("#adet").textContent = M.kapiSayisi(sonuc.length, kapilar.length);
  const ogeler = sonuc.map(k => h("div", { class: "oge", role: "button", tabindex: "0", "data-id": metin(k.id), "aria-current": secili === k ? "true" : "false" },
    h("span", { class: yontemSinifi(k.yontem) }, k.yontem),
    h("span", { class: "yol mono" }, k.yol),
    h("span", { class: "alt" }, k.ozet || [addanOku(k.fonksiyon), " ", h("span", { class: "rozet uyari", title: M.ozetYok }, M.guven.inferred)], k.kullanimdan_kalkmis === true ? M.kalkmisEk : "", k.ana === false ? M.ikincilEk : "")));
  yerlestir($("#liste"), ogeler.length ? ogeler : h("div", { class: "bos" }, M.eslesenKapiYok));
}

$("#liste").addEventListener("click", e => { const o = e.target.closest(".oge"); if (o) sec(o.dataset.id); });
$("#liste").addEventListener("keydown", e => { const o = e.target.closest(".oge"); if (o && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); sec(o.dataset.id); } });
for (const id of ["#ara", "#s-yontem", "#s-giris", "#s-diger"]) $(id).addEventListener("input", listele);
$("#ara").addEventListener("input", sunucuAra);

function zorunlulukRozeti(a) {
  if (a.belirsiz === true) return h("span", { class: "rozet kotu" }, M.zorunluBelirsiz);
  return a.zorunlu === true ? h("span", { class: "rozet uyari" }, M.zorunlu) : h("span", { class: "rozet" }, M.istegeBagli);
}

function alanTablosu(baslik, g) {
  g = nesne(g);
  if (!g.sema) return [h("h4", null, baslik), h("p", { class: "mono" }, g.ad || M.tamami, " ", g.tip || "")];
  const s = nesne(g.sema);
  const ust = h("h4", null, baslik, " · ", h("span", { class: "mono dto-ad" }, s.dto));
  if (s.bulunamadi === true) return [ust, h("p", null, h("span", { class: "rozet kotu" }, M.semaCozulemedi), " ", h("span", { class: "soluk" }, M.semaCozulemediNot))];
  const satirlar = Object.entries(nesne(s.alanlar)).map(([ad, a]) => {
    a = nesne(a);
    const kurallar = dizi(a.kurallar).map(metin).filter(r => !/^(IsOptional|of)\(/.test(r)).join("  ");
    return h("tr", null,
      h("td", { class: "mono" }, ad),
      h("td", null, zorunlulukRozeti(a)),
      h("td", { class: "mono" }, a.tip || "?"),
      h("td", { class: "mono" }, kurallar, Array.isArray(a.degerler) ? h("div", { class: "soluk" }, M.degerler, a.degerler.map(metin).join(" | ")) : null));
  });
  return [ust,
    h("div", { class: "tablo-kap" }, h("table", null,
      h("thead", null, h("tr", null, h("th", null, M.alan), h("th"), h("th", null, M.tip), h("th", null, M.kurallar))),
      h("tbody", null, satirlar))),
    h("p", { class: "soluk mono dosya-satiri" }, s.dosya || "")];
}

function iskelet(k, bolum) {
  const o = {};
  for (const g of dizi(k[bolum]).map(nesne)) {
    const s = nesne(g.sema);
    if (g.sema && s.bulunamadi !== true) for (const [ad, a0] of Object.entries(nesne(s.alanlar))) {
      const a = nesne(a0);
      if (a.zorunlu !== true) continue;
      const t = metin(a.tip).toLowerCase();
      o[ad] = Array.isArray(a.degerler) ? a.degerler[0] : /int|number/.test(t) ? 0 : /bool/.test(t) ? false : /\[\]|array/.test(t) ? [] : "";
    } else if (g.ad) o[metin(g.ad)] = "";
  }
  return Object.keys(o).length ? JSON.stringify(o, null, 2) : "";
}

function sec(id) {
  secili = kapilar.find(k => k.id === id);
  if (!secili) return;
  history.replaceState(null, "", "#" + encodeURIComponent(id));
  listele();
  const k = secili;
  // Etki bölümleri sunucudan (çağrı zinciri, tablo etkisi, yetki, dış etki, ön koşul, geri alma, onay)
  const etkiKutusu = h("div", { class: "etki", "aria-live": "polite" });
  // Kapının önermeleri ve güvenleri: tani ile aynı kanıt modelinden
  const onermeler = dizi(nesne(kanit)[k.id]).map(nesne).filter(o => ["GIRIS", "ROLLER", "GOVDE", "SORGU", "VERI"].includes(o.yuklem) || o.gecerlilik !== "gecerli");
  const kanitBolumu = onermeler.length ? h("details", { class: "kanit-ayrinti" },
    h("summary", null, M.kanitBaslik),
    h("div", { class: "satir-rozet" }, onermeler.map(o => h("span", { class: GUVEN_SINIFI[o.gecerlilik === "gecerli" ? o.guven : "eski"] || "rozet" },
      M.onermeAdi[o.yuklem] || o.yuklem, o.hedef && !["ROLLER", "VERI", "GIRIS"].includes(o.yuklem) ? " " + metin(o.hedef) : "", " · ",
      o.gecerlilik === "gecerli" ? (M.guven[o.guven] || metin(o.guven)) : M.eskiDosya, o.kapsam === "acik" ? M.listeAcik : ""))),
    h("p", { class: "soluk bosluksuz" }, M.kanitNot)) : null;
  const girisSinifi = k.giris === "gerekli" ? "rozet vurgu" : k.giris === "herkese_acik" ? "rozet iyi" : "rozet";
  const sonuc = h("pre", { class: "sonuc", id: "o-sonuc", hidden: "" });
  const kapiKutusu = h("input", { type: "text", id: "o-kapi", class: "mono", "aria-label": M.kapi });
  kapiKutusu.value = metin(k.id);
  const govdeKutusu = h("textarea", { id: "o-govde", rows: "7", placeholder: M.govdeJson, "aria-label": M.govde });
  govdeKutusu.value = iskelet(k, "govde");
  const sorguKutusu = h("textarea", { id: "o-sorgu", rows: "7", placeholder: M.sorguJson, "aria-label": M.sorgu });
  sorguKutusu.value = iskelet(k, "sorgu");
  const oturum = h("input", { type: "checkbox", id: "o-oturum" });
  oturum.checked = true;
  yerlestir($("#ayrinti"),
    h("h2", null, h("span", { class: yontemSinifi(k.yontem) }, k.yontem), " ", h("span", { class: "mono" }, k.yol)),
    h("div", { class: "soluk" }, k.ozet || [addanOku(k.fonksiyon), " ", h("span", { class: "rozet uyari", title: M.ozetYok }, M.ozetYokRozet)]),
    h("div", { class: "satir-rozet" },
      h("span", { class: girisSinifi }, M.giris[k.giris] || k.giris),
      k.giris_kaniti && k.giris_kaniti !== "kesin" ? h("span", { class: "rozet uyari" }, M.girisCikarim) : null,
      dizi(k.roller).map(r => h("span", { class: "rozet vurgu" }, M.rol, r)),
      k.surum ? h("span", { class: "rozet" }, M.surum, k.surum) : null,
      k.kullanimdan_kalkmis === true ? h("span", { class: "rozet kotu" }, M.kalkmis, k.yerine ? " → " + metin(k.yerine) : "") : null,
      k.ana === false ? h("span", { class: "rozet uyari" }, M.ikincil, k.uygulama) : null),
    dizi(k.guardlar).length ? h("div", { class: "mono soluk" }, M.korumalar, dizi(k.guardlar).map(metin).join(", ")) : null,
    etkiKutusu,
    h("h3", null, M.girdiler),
    dizi(k.parametreler).length ? [h("h4", null, M.yolParametreleri), h("p", { class: "mono" }, dizi(k.parametreler).map(p => typeof p === "string" ? p : metin(nesne(p).ad || nesne(p).tip)).join(", "))] : null,
    dizi(k.govde).map(g => alanTablosu(M.govde, g)),
    dizi(k.sorgu).map(g => alanTablosu(M.sorgu, g)),
    girdiler(k).length || dizi(k.parametreler).length ? null : h("p", { class: "soluk" }, M.girdiYok),
    kanitBolumu,
    h("h3", null, M.onizle),
    h("div", { class: "onizle" },
      h("p", { class: "soluk bosluksuz" }, M.onizleNot),
      kapiKutusu,
      h("div", { class: "iki" }, govdeKutusu, sorguKutusu),
      h("div", { class: "iki" },
        h("label", { class: "kutu" }, oturum, M.girisYapmis),
        h("input", { type: "text", id: "o-roller", placeholder: M.rollerIpucu, "aria-label": M.roller })),
      h("button", { class: "ana", id: "o-git", onclick: onizle }, M.onizle),
      sonuc));
  islemAyrintisi(k.id, etkiKutusu);
  // Dar ekranda ayrıntı listenin altında: seçilince görünür olsun
  if (matchMedia("(max-width: 820px)").matches) $("#ayrinti").scrollIntoView({ block: "start" });
}

async function onizle() {
  const cikti = $("#o-sonuc");
  cikti.hidden = false;
  const oku = (id, ad) => { const t = $(id).value.trim(); if (!t) return undefined; try { return JSON.parse(t); } catch (e) { throw new Error(M.gecersizJson(ad, e.message)); } };
  try {
    const roller = $("#o-roller").value.split(",").map(s => s.trim()).filter(Boolean);
    const istek = { kapi: $("#o-kapi").value, govde: oku("#o-govde", M.govde), sorgu: oku("#o-sorgu", M.sorgu), oturum: $("#o-oturum").checked, ...(roller.length ? { roller } : {}) };
    const c = await api("/api/onizle", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(istek) });
    cikti.textContent = metin(nesne(await c.json()).metin);
  } catch (e) { cikti.textContent = e.message; }
}

function liste(xs, f) {
  return xs.length ? h("ul", null, xs.map(f)) : null;
}

function kapsamCiz() {
  const r = nesne(rapor), g = nesne(r.girdiler), v = nesne(r.veri), ok = nesne(r.okunan), kp = nesne(r.kapilar), kn = nesne(r.kanit);
  const coz = dizi(g.cozulemeyen_semalar).map(nesne);
  const den = Object.entries(nesne(g.onizlemede_kontrol_edilemeyen_kurallar)).sort((a, b) => Number(b[1]) - Number(a[1]));
  const kismi = dizi(ok.kismen_okunan_dosyalar);
  const semalar = dizi(v.sema_dosyalari);
  const bagsiz = dizi(v.bagsiz_kapilar);
  const kalkmis = dizi(r.kullanimdan_kalkmis).map(nesne);
  const kardes = dizi(ok.kardes_paketler);
  const okunamayan = dizi(kp.okunamayan_rota_kaynaklari).map(metin);
  yerlestir($("#kapsam"),
    h("div", { class: "panel" }, h("h3", null, M.girdiler),
      h("p", null, M.girdiliKapi(metin(g.girdili_kapi), metin(g.alan), metin(g.kural))),
      Number(g.zorunlulugu_belirsiz_alan) > 0 ? h("p", null, h("span", { class: "rozet uyari" }, M.belirsizAlan(metin(g.zorunlulugu_belirsiz_alan))), M.belirsizAlanNot) : null,
      coz.length
        ? [h("p", null, h("span", { class: "rozet kotu" }, M.cozulemeyenSema(coz.length)), M.cozulemeyenSemaNot), liste(coz, c => h("li", { class: "mono" }, metin(c.kapi), " · ", metin(c.sema)))]
        : h("p", null, h("span", { class: "rozet iyi" }, M.butunSemalar)),
      den.length ? h("p", { class: "soluk" }, M.denetlenemeyenKurallar, den.map(([k, n], i) => [i ? ", " : "", h("span", { class: "mono" }, k), " ", metin(n)])) : null),
    h("div", { class: "panel" }, h("h3", null, M.veriBaglari),
      v.bagli_kapi !== null && v.bagli_kapi !== undefined
        ? [h("p", null, M.bagliKapi(metin(v.bagli_kapi), metin(kp.ana)),
            semalar.length ? [M.sema, h("span", { class: "mono" }, semalar.map(metin).join(", "))] : M.tabloSorgudan),
          bagsiz.length ? h("details", null, h("summary", null, M.bagsizKapi(bagsiz.length)), liste(bagsiz, b => h("li", { class: "mono" }, b))) : null]
        : h("p", { class: "soluk" }, M.veriYok)),
    kn.onerme !== undefined ? h("div", { class: "panel" }, h("h3", null, M.guvenBolum),
      h("p", null, M.onermeSayisi(metin(kn.onerme)), ["certain", "observed", "inferred", "unknown"].map(g => `${metin(nesne(kn.guven)[g])} ${M.guven[g]}`).join(" · "), M.kanitRev, metin(kn.revizyon)),
      Number(kn.eski) > 0 ? h("p", null, h("span", { class: "rozet kotu" }, M.eskiOnerme(metin(kn.eski))), M.eskiOnermeNot) : null,
      Number(kn.celiski) > 0 ? h("p", null, h("span", { class: "rozet uyari" }, M.celiski(metin(kn.celiski))), M.celiskiNot) : null) : null,
    h("div", { class: "panel" }, h("h3", null, M.kapilar),
      h("p", null, M.anaKapi(metin(kp.ana)), Object.entries(nesne(kp.giris)).map(([k, n]) => `${M.giris[k] || k} ${metin(n)}`).join(" · "), M.rolKisitli(metin(kp.rol_kisitli))),
      liste(Object.entries(nesne(kp.ikincil_uygulamalar)), ([a, n]) => h("li", null, M.ikincilUygulama, h("b", null, a), M.ikincilKapi(metin(n)))),
      kalkmis.length ? [h("p", null, M.kalkmisKapi(kalkmis.length)), liste(kalkmis, k => h("li", { class: "mono" }, metin(k.kapi), k.yerine ? " → " + metin(k.yerine) : ""))] : null,
      okunamayan.length ? [h("p", null, h("span", { class: "rozet uyari" }, M.okunamayanRota(okunamayan.length)), M.okunamayanRotaNot), liste(okunamayan.slice(0, 20), x => h("li", { class: "mono" }, x))] : null),
    h("div", { class: "panel" }, h("h3", null, M.okuma),
      h("p", null, M.dosyaSatir(metin(ok.dosya), metin(ok.satir)), kardes.length ? [M.kardes, h("span", { class: "mono" }, kardes.map(metin).join(", "))] : null),
      kismi.length
        ? [h("p", null, h("span", { class: "rozet uyari" }, M.kismenOkunan(kismi.length)), M.kismenOkunanNot), liste(kismi, d => h("li", { class: "mono" }, d))]
        : h("p", null, h("span", { class: "rozet iyi" }, M.tamOkundu))),
    h("p", { class: "soluk" }, M.temizRapor));
}

function imzaCiz() {
  const e = $("#imza");
  const i = nesne(imza);
  const t = M.imza;
  // Yeşil (güvenilir) rozet yalnızca kullanıcının sabitlediği anahtarla: yanındaki dosyayla tutarlılık kimlik kanıtı değildir
  const m = {
    guvenilir: ["iyi", t.guvenilir(metin(i.alg), metin(i.key_id))],
    sabitlenmis_anahtarla_uyusmuyor: ["kotu", t.uyusmuyor(metin(i.key_id))],
    anahtar_degisti: ["uyari", t.degisti(metin(i.key_id))],
    tutarli_anahtar_dogrulanmadi: ["", t.tutarli(metin(i.alg), metin(i.key_id))],
    gecersiz: ["kotu", t.gecersiz],
    anahtar_yok: ["uyari", t.anahtarYok],
    imzasiz: ["", t.imzasiz],
  }[i.durum] || ["", metin(i.durum)];
  e.className = "rozet " + m[0];
  if (i.durum === "anahtar_degisti") e.title = t.degistiBaslik;
  if (i.durum === "tutarli_anahtar_dogrulanmadi") e.title = t.tutarliBaslik;
  e.textContent = m[1];
  if (i.hata) e.title = metin(i.hata);
}

// ---- Sezgi (tanıtım) ve "Bu uygulama hakkında ne biliyoruz" ----

const DURUM = Object.fromEntries(Object.entries(M.durum).map(([k, v]) => [k, [v, { hazir: "rozet iyi", kurulmamis: "rozet uyari", bilinmiyor: "rozet kotu" }[k] || "rozet"]]));
const GUVEN_AD = M.guven;
const YANIT = M.yanit;
const guvenRozeti = g => h("span", { class: (GUVEN_SINIFI[g] || "rozet") + " damga", title: M.guvenBaslik + metin(g) }, GUVEN_AD[g] || metin(g));
/** Damgaların en zayıfı (certain > observed > inferred > unknown) */
const enZayif = gs => ["unknown", "inferred", "observed", "certain"].find(g => gs.includes(g)) || "unknown";
const GERI_SINIFI = { evet: "rozet iyi", kismen: "rozet uyari", hayir: "rozet kotu", bilinmiyor: "rozet", okuma: "rozet" };
const ONAY_SINIFI = { gerekmez: "rozet iyi", kosullu: "rozet uyari", her_seferinde: "rozet kotu" };
const RISK_SINIFI = { okuma: "rozet", yazma: "rozet uyari", silme: "rozet kotu" };

function komutSatiri(k) {
  return h("div", { class: "komut-satiri" }, h("code", { class: "mono" }, metin(k)), kopyaDugmesi(metin(k)));
}

/** Güven damgalı madde listesi (soru yanıtı ve işlem ayrıntısı aynı biçimde) */
function maddeListesi(maddeler) {
  return h("ul", { class: "maddeler" }, maddeler.map(nesne).map(m => h("li", null, guvenRozeti(m.guven), h("span", null, m.metin,
    dizi(m.ornekler).length ? h("span", { class: "ornekler" }, dizi(m.ornekler).map(o => h("span", { class: "mono ornek" }, o))) : null))));
}

function anlamCiz(a) {
  const kutu = $("#anlam");
  if (!a || typeof a !== "object") { kutu.hidden = true; return; }
  kutu.hidden = false;
  const alan = nesne(a.alan), amac = nesne(a.amac), problem = nesne(a.problem), yet = nesne(a.yetenek);
  const satir = (baslik, ...icerik) => h("div", { class: "anlam-satir" }, h("dt", null, baslik), h("dd", null, ...icerik));
  yerlestir(kutu,
    h("p", { class: "ust-baslik", id: "anlam-baslik" }, M.anlamBaslik),
    h("dl", { class: "anlam-liste" },
      satir(M.alanBaslik, alan.ad ? [guvenRozeti(alan.guven), " ", h("b", null, alan.ad), nesne(a.tur).ad ? h("span", { class: "soluk" }, " · ", a.tur.ad) : null,
        dizi(alan.kanit).length ? h("span", { class: "ornekler" }, dizi(alan.kanit).map(o => h("span", { class: "mono ornek" }, o))) : null]
        : [guvenRozeti("unknown"), " ", M.alanYok, alan.not ? h("span", { class: "soluk" }, " (", alan.not, ")") : null]),
      satir(M.amac, amac.alinti ? [guvenRozeti(amac.guven), " ", h("q", { class: "alinti" }, amac.alinti), " ", h("span", { class: "mono soluk" }, amac.konum)]
        : [guvenRozeti("unknown"), " ", h("span", { class: "soluk" }, M.amacYok)]),
      problem.metin ? satir(M.problem, guvenRozeti(problem.guven), " ", problem.metin) : null,
      dizi(a.aktorler).length ? satir(M.kullanicilar, guvenRozeti(enZayif(dizi(a.aktorler).map(x => nesne(x).guven))), " ", h("span", { class: "satir-rozet bosluksuz satir-ici" }, dizi(a.aktorler).map(nesne).map(x =>
        h("span", { class: "rozet", title: M.guvenBaslik + metin(x.guven) }, metin(x.ad), " · ", M.islemSayisi(metin(x.islem)))))) : null,
      Number(yet.sayi) > 0 ? satir(M.yetenekler(metin(yet.sayi)), guvenRozeti(yet.guven), " ", h("span", { class: "ornekler satir-ici" }, dizi(yet.ornekler).map(o => h("span", { class: "ornek" }, o)))) : null),
    h("p", { class: "soluk dipnot" }, M.anlamNot));
}

let araSayaci = 0;
async function gorevAra(q) {
  const kutu = $("#ara-sonuc");
  q = q.trim();
  if (!q) { yerlestir(kutu); return; }
  const no = ++araSayaci;
  yerlestir(kutu, h("p", { class: "soluk" }, M.araniyor));
  let v;
  try { v = nesne(await (await api("/api/ara", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ sorgu: q }) })).json()); }
  catch (e) { if (no === araSayaci) yerlestir(kutu, h("p", { class: "rozet kotu" }, metin(e.message))); return; }
  if (no !== araSayaci) return;
  const sonuclar = dizi(v.sonuclar).map(nesne), toplam = dizi(v.kapilar).length;
  yerlestir(kutu,
    v.plan ? h("details", { class: "plan", open: "" }, h("summary", null, M.plan), h("pre", { class: "sonuc" }, metin(v.plan))) : null,
    sonuclar.length ? [h("p", { class: "soluk bosluksuz" }, M.sonucSayisi(toplam, sonuclar.length)),
      h("ul", { class: "ara-sonuclari" }, sonuclar.map(s => {
        const g = nesne(s.geri), o = nesne(s.onay);
        return h("li", { class: "ara-oge" },
          h("div", { class: "ara-ust" },
            h("span", { class: yontemSinifi(s.yontem) }, s.yontem), " ", h("span", { class: "mono yol" }, s.yol),
            h("button", { class: "ikincil", type: "button", onclick: () => kapiyaGit(metin(s.id)) }, M.ayrinti, " →")),
          h("div", null, s.aciklama, s.aciklama_kesin === true ? null : [" ", h("span", { class: "rozet uyari", title: M.ozetYok }, M.guven.inferred)]),
          h("div", { class: "satir-rozet" },
            h("span", { class: RISK_SINIFI[s.risk] || "rozet" }, M.risk[s.risk] || metin(s.risk)),
            dizi(s.etki).map(e => h("span", { class: "rozet" }, e)),
            h("span", { class: GERI_SINIFI[g.karar] || "rozet" }, M.geri[g.karar] || metin(g.karar)),
            h("span", { class: ONAY_SINIFI[o.karar] || "rozet", title: metin(o.metin) }, M.onay[o.karar] || metin(o.karar))),
          h("p", { class: "soluk bosluksuz" }, o.metin));
      }))]
      : (v.plan ? null : h("p", { class: "soluk" }, M.sonucYok(q))));
}
let araZamani = null;
$("#ara-form").addEventListener("submit", e => { e.preventDefault(); clearTimeout(araZamani); gorevAra($("#ara-gorev").value); });
$("#ara-gorev").addEventListener("input", () => { clearTimeout(araZamani); araZamani = setTimeout(() => gorevAra($("#ara-gorev").value), 400); });

/** Arama sonucundan Kapılar sekmesinde işleme git */
function kapiyaGit(id) {
  sekmeAc("kapilar");
  sec(id);
  $("#ayrinti").focus();
}

function yetenekKarti(y) {
  const d = DURUM[y.durum] || [metin(y.durum), "rozet"];
  const id = "yt-" + metin(y.id).replace(/[^a-z0-9_-]/gi, "");
  const ana = y.grup === "ana";
  return h("article", { class: "panel yetenek durum-" + metin(y.durum).replace(/[^a-z]/g, "") + (ana ? " ana-kart" : ""), role: "listitem", "aria-labelledby": id },
    h("div", { class: "yetenek-ust" },
      h("h3", { id }, ana && Number(y.adim) > 0 ? h("span", { class: "adim", "aria-hidden": "true" }, metin(y.adim)) : null, y.baslik),
      h("span", { class: d[1] }, d[0])),
    h("p", { class: "bosluksuz" }, y.aciklama),
    dizi(y.durumlar).length
      ? h("ul", { class: "alt-durumlar" }, dizi(y.durumlar).map(nesne).map(a => h("li", { class: "alt-" + metin(a.durum).replace(/[^a-z]/g, "") },
        h("span", { class: "simge", "aria-hidden": "true" }, M.altDurumSimge[a.durum] || "·"), h("span", { class: "gizli" }, (M.durum[a.durum] || metin(a.durum)) + ": "), a.metin)))
      : h("p", { class: "yetenek-durum" }, y.ayrinti),
    komutSatiri(y.komut));
}

function tanitimCiz(t) {
  const haritali = t.harita === true;
  yerlestir($("#proje-durum"),
    t.klasor ? h("span", { class: "rozet" }, M.klasor, t.klasor) : null,
    haritali
      ? [h("span", { class: "rozet vurgu" }, metin(t.uygulama)),
        t.kaynaksiz === true ? h("span", { class: "rozet" }, M.kaynaksiz) : h("span", { class: "rozet" }, metin(t.islem), M.islemEk),
        Number(t.degisen) > 0 ? h("span", { class: "rozet uyari" }, M.degisen(metin(t.degisen))) : h("span", { class: "rozet iyi" }, M.haritaGuncel)]
      : [h("span", { class: "rozet uyari" }, M.haritaYok), t.cati ? h("span", { class: "rozet" }, M.cati, t.cati) : h("span", { class: "rozet" }, M.catiYok)]);
  anlamCiz(haritali ? t.anlam : null);
  $("#ara-panel").hidden = !haritali || t.kaynaksiz === true;
  const s = nesne(t.sonraki);
  const kutu = $("#sonraki");
  kutu.hidden = false;
  yerlestir(kutu, h("p", { class: "ust-baslik" }, M.sonraki), s.komut
    ? [komutSatiri(s.komut), h("p", { class: "soluk bosluksuz" }, metin(s.aciklama))]
    : h("p", { class: "bosluksuz" }, h("span", { class: "rozet iyi" }, M.kurulumTamam), M.kurulumTamamMetin));
  const ys = dizi(t.yetenekler).map(nesne);
  yerlestir($("#yetenekler"), ys.filter(y => y.grup === "ana").map(yetenekKarti));
  yerlestir($("#diger-yetenekler"), ys.filter(y => y.grup !== "ana").map(yetenekKarti));
}

function bilgiCiz(b) {
  yerlestir($("#olcek"), dizi(b.olcek).map(nesne).map((o, i) =>
    h("span", { class: "olcek-oge" }, i ? h("span", { class: "soluk", "aria-hidden": "true" }, "›") : null, guvenRozeti(o.guven), h("span", { class: "soluk" }, metin(o.anlam)))));
  const sorular = dizi(b.sorular).map(nesne);
  const say = d => sorular.filter(s => s.durum === d).length;
  yerlestir($("#bilgi-ozet"),
    h("span", { class: "rozet" }, M.soru(sorular.length)),
    h("span", { class: "rozet iyi" }, M.yanitliSayi(say("yanitli"))),
    h("span", { class: "rozet uyari" }, M.kismenSayi(say("kismen"))),
    h("span", { class: "rozet kotu" }, M.bilinmiyorSayi(say("bilinmiyor"))));
  yerlestir($("#sorular"), sorular.map(s => {
    const k = nesne(s.komut);
    const maddeler = dizi(s.maddeler).map(nesne);
    const id = "sr-" + metin(s.id).replace(/[^a-z0-9_-]/gi, "");
    return h("article", { class: "panel soru yanit-" + metin(s.durum).replace(/[^a-z]/g, ""), "aria-labelledby": id },
      h("div", { class: "soru-ust" },
        h("h3", { id }, Number(s.no) > 0 ? `${metin(s.no)}. ` : "", s.soru),
        h("span", { class: "soru-rozetler" }, h("span", { class: "rozet" }, YANIT[s.durum] || metin(s.durum)), guvenRozeti(s.guven))),
      maddeler.length ? maddeListesi(maddeler) : null,
      s.bilinmeyen ? h("p", { class: "bilinmeyen" }, h("b", null, maddeler.length ? M.bilinmeyen : M.bilinmiyor), s.bilinmeyen) : null,
      k.komut ? h("div", { class: "yardim" }, h("span", { class: "soluk" }, M.yardimci, metin(k.aciklama)), komutSatiri(k.komut)) : null);
  }));
}

/** Kapı ayrıntısının etki bölümleri (`/api/islem`): çağrı zinciri, tablo, yetki, dış etki, ön koşul, geri alma, onay */
async function islemAyrintisi(id, hedef) {
  yerlestir(hedef, h("p", { class: "soluk" }, M.islemYukleniyor));
  let v;
  try { v = nesne(await (await api("/api/islem", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ kapi: id }) })).json()); }
  catch (_) { v = {}; }
  if (!secili || secili.id !== id) return;
  if (!dizi(v.bolumler).length) { yerlestir(hedef, h("p", { class: "soluk" }, M.islemAlinamadi)); return; }
  const g = nesne(v.geri), o = nesne(v.onay);
  yerlestir(hedef,
    h("div", { class: "satir-rozet" },
      h("span", { class: RISK_SINIFI[v.risk] || "rozet" }, M.risk[v.risk] || metin(v.risk)),
      h("span", { class: GERI_SINIFI[g.karar] || "rozet" }, M.geri[g.karar] || metin(g.karar)),
      h("span", { class: ONAY_SINIFI[o.karar] || "rozet" }, M.onay[o.karar] || metin(o.karar))),
    dizi(v.bolumler).map(nesne).map(b => h("section", { class: "etki-bolum", "aria-label": metin(b.baslik) },
      h("h3", null, b.baslik),
      dizi(b.maddeler).length ? maddeListesi(dizi(b.maddeler)) : null,
      b.bilinmeyen ? h("p", { class: "bilinmeyen" }, h("b", null, dizi(b.maddeler).length ? M.bilinmeyen : M.bilinmiyor), b.bilinmeyen) : null)));
}

// ---- İzinler ve token ----
// Seçim işlem kimliği + risk olarak tutulur ("album.add-assets:yaz"); token bu açık listeyle üretilir (joker yok:
// sonradan eklenen işlemler kendiliğinden açılmaz).

const RISK = { oku: [M.izinRisk.oku, ""], yaz: [M.izinRisk.yaz, "uyari"], yikici: [M.izinRisk.yikici, "kotu"] };
const izin = { yuklendi: false, islemler: [], secili: new Set() };
const anahtar = i => `${metin(i.id)}:${metin(i.risk)}`;

async function izinYukle() {
  const v = nesne(await (await api("/api/izinler")).json());
  izin.yuklendi = true;
  izin.islemler = dizi(v.islemler).map(nesne);
  $("#iz-mod").textContent = v.hata ? M.tokenDeposu(metin(v.hata)) : M.mod[v.mod] || metin(v.mod);
  izinCiz();
  tokenlerCiz(dizi(v.tokenler).map(nesne));
}

function izinSay() {
  const n = izin.secili.size;
  $("#iz-adet").textContent = M.seciliIslem(n);
  $("#iz-uret").disabled = n === 0;
  $("#iz-uret").textContent = n ? M.tokenUretN(n) : M.tokenUret;
}

function islemSatiri(i) {
  const kutu = h("input", { type: "checkbox", "aria-label": metin(i.kapi) });
  kutu.checked = izin.secili.has(anahtar(i));
  kutu.addEventListener("change", () => { kutu.checked ? izin.secili.add(anahtar(i)) : izin.secili.delete(anahtar(i)); izinSay(); });
  const r = RISK[i.risk] || [metin(i.risk), ""];
  return h("label", { class: "islem" }, kutu,
    h("span", { class: yontemSinifi(i.yontem) }, i.yontem),
    h("span", null,
      h("span", { class: "mono" }, i.yol), " ",
      h("span", { class: "rozet " + r[1] }, r[0]),
      i.kalkmis ? h("span", { class: "rozet kotu" }, M.kalkmisKisa) : null,
      h("div", null, i.aciklama || "", i.aciklama_kesin ? null : [" ", h("span", { class: "rozet uyari", title: M.ozetYok }, M.guven.inferred)]),
      h("div", { class: "soluk" },
        M.giris[i.giris] || metin(i.giris),
        dizi(i.roller).length ? M.rolListe + dizi(i.roller).map(metin).join(", ") : "",
        dizi(i.okur).length ? M.okur + dizi(i.okur).map(metin).join(", ") : "",
        dizi(i.yazar).length ? M.yazar + dizi(i.yazar).map(metin).join(", ") : "",
        " · ", h("span", { class: "mono" }, i.id))));
}

function izinCiz() {
  const q = $("#iz-ara").value.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const uyan = izin.islemler.filter(i => q.every(w => [i.modul, i.yol, i.yontem, i.aciklama, i.id, ...dizi(i.okur), ...dizi(i.yazar)].map(metin).join(" ").toLowerCase().includes(w)));
  const moduller = new Map();
  for (const i of uyan) { const m = metin(i.modul) || M.adsiz; if (!moduller.has(m)) moduller.set(m, []); moduller.get(m).push(i); }
  const bolumler = [...moduller.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([m, xs]) => {
    // Modül düzeyinde seviye düğmeleri: o modülde o riskteki bütün işlemleri seçer ya da bırakır
    const dugmeler = ["oku", "yaz", "yikici"].filter(r => xs.some(i => i.risk === r)).map(r => h("button", { class: "ikincil", onclick: e => {
      e.preventDefault();
      const ilgili = xs.filter(i => i.risk === r);
      const hepsi = ilgili.every(i => izin.secili.has(anahtar(i)));
      for (const i of ilgili) hepsi ? izin.secili.delete(anahtar(i)) : izin.secili.add(anahtar(i));
      izinCiz();
    } }, (RISK[r] || [r])[0]));
    const secilen = xs.filter(i => izin.secili.has(anahtar(i))).length;
    return h("details", { class: "modul", open: q.length ? "" : null },
      h("summary", null, h("b", null, m), " ", h("span", { class: "soluk" }, M.modulIslem(xs.length), secilen ? M.modulSecili(secilen) : ""), h("span", { class: "dugmeler" }, dugmeler)),
      xs.map(islemSatiri));
  });
  yerlestir($("#iz-moduller"), bolumler.length ? bolumler : h("div", { class: "bos" }, M.eslesenIslemYok));
  izinSay();
}

$("#iz-ara").addEventListener("input", izinCiz);
$("#iz-okumalar").addEventListener("click", () => { for (const i of izin.islemler) if (i.risk === "oku") izin.secili.add(anahtar(i)); izinCiz(); });
$("#iz-temizle").addEventListener("click", () => { izin.secili.clear(); izinCiz(); });

function kopyaDugmesi(deger) {
  const b = h("button", { class: "ikincil" }, M.kopyala);
  b.addEventListener("click", async () => { try { await navigator.clipboard.writeText(deger); b.textContent = M.kopyalandi; } catch (_) { b.textContent = M.kopyalanamadi; } });
  return b;
}

$("#iz-uret").addEventListener("click", async () => {
  const kutu = $("#iz-sonuc");
  kutu.hidden = false;
  const istek = { ad: $("#iz-ad").value.trim(), izinler: [...izin.secili], sure: Number($("#iz-sure").value), onay: $("#iz-onay").value };
  if (!istek.ad) { yerlestir(kutu, h("p", { class: "rozet kotu" }, M.tokenAdi)); return; }
  try {
    const v = nesne(await (await fetch(dilli("/api/token"), { method: "POST", headers: yazmaBasliklari(), body: JSON.stringify(istek) })).json());
    if (v.hata) { yerlestir(kutu, h("p", { class: "rozet kotu" }, metin(v.hata))); return; }
    const token = metin(v.token);
    yerlestir(kutu,
      h("p", null, h("span", { class: "rozet iyi" }, M.tokenUretildi), " ", h("b", null, M.birDaha), M.simdiKopyala),
      h("div", { class: "kopya" }, h("pre", { class: "sonuc mono" }, token), kopyaDugmesi(token)),
      h("h3", null, M.araclar),
      dizi(v.araclar).length
        ? [h("p", { class: "soluk" }, M.araclarNot(dizi(v.araclar).length)),
          h("ul", { class: "arac-listesi" }, dizi(v.araclar).map(nesne).map(t => h("li", null, h("b", { class: "mono" }, t.ad), h("div", { class: "soluk" }, t.aciklama))))]
        : h("p", { class: "soluk" }, M.esikNot(metin(v.gorunen), metin(v.esik))),
      h("h3", null, M.bagla),
      dizi(v.baglanti).map(nesne).map(b => h("div", null, h("div", { class: "soluk" }, metin(b.baslik)), h("div", { class: "kopya" }, h("pre", { class: "sonuc mono" }, metin(b.satir)), kopyaDugmesi(metin(b.satir))))),
      h("p", { class: "soluk" }, M.mod[v.mod] || ""));
    izin.secili.clear();
    $("#iz-ad").value = "";
    izinYukle();
  } catch (e) { yerlestir(kutu, h("p", { class: "rozet kotu" }, metin(e.message))); }
});

function tokenlerCiz(liste) {
  const DURUM = { gecerli: [M.tokenDurum.gecerli, "iyi"], iptal: [M.tokenDurum.iptal, "kotu"], suresi_dolmus: [M.tokenDurum.suresi_dolmus, "uyari"] };
  const satirlar = liste.map(t => {
    const d = DURUM[t.durum] || [metin(t.durum), ""];
    const izinler = dizi(t.izinler).map(nesne);
    const iptal = t.durum === "gecerli" ? h("button", { class: "ikincil", onclick: async () => {
      if (!confirm(M.iptalSoru(metin(t.ad)))) return;
      const v = nesne(await (await fetch(dilli("/api/token/iptal"), { method: "POST", headers: yazmaBasliklari(), body: JSON.stringify({ ad: t.ad }) })).json());
      if (v.hata) alert(metin(v.hata)); else izinYukle();
    } }, M.iptalEt) : null;
    const bitis = Number(t.bitis) ? new Date(Number(t.bitis) * 1000).toLocaleDateString(DIL) : "";
    return h("div", { class: "token-satiri" },
      h("div", null, h("b", null, t.ad), " ", h("span", { class: "rozet " + d[1] }, d[0]), " ", h("span", { class: "soluk" }, M.tokenOzet(izinler.length, metin(t.onay)), bitis ? M.bitis(bitis) : "")),
      h("details", null, h("summary", { class: "soluk" }, M.izinler), h("ul", { class: "mono" }, izinler.map(i => h("li", null, metin(i.desen), " : ", metin(i.seviye))))),
      iptal);
  });
  yerlestir($("#iz-tokenler"), satirlar.length ? satirlar : h("p", { class: "soluk" }, M.tokenYok));
}

(async () => {
  if (yonlendiriliyor) return;
  const al = async u => {
    const c = await api(u);
    if (c.status === 403) throw new Error(M.anahtarsiz);
    return c.json();
  };
  const hata = e => document.querySelector("main").prepend(h("p", { class: "rozet kotu uyari-satiri", role: "alert" }, metin(e.message)));
  // Önce tanıtım ve sorular (haritasız açılışta da var), sonra harita verisi
  let tanit, bilgi;
  try {
    [tanit, bilgi] = await Promise.all([al("/api/sezgi"), al("/api/sorular")]);
  } catch (e) {
    hata(e);
    return;
  }
  tanit = nesne(tanit);
  tanitimCiz(tanit);
  bilgiCiz(nesne(bilgi));
  if (tanit.harita !== true) {
    // Haritasız açılış: yalnızca tanıtım. `sezgi init` haritayı yazınca sayfa kendini yeniler
    $("#uygulama").textContent = M.haritaYokBaslik;
    const bekle = setInterval(async () => {
      try {
        if (nesne(await al("/api/sezgi")).harita === true) { clearInterval(bekle); location.reload(); }
      } catch (_) { clearInterval(bekle); }
    }, 3000);
    return;
  }
  for (const b of document.querySelectorAll("nav [data-harita]")) b.hidden = false;
  $("#sayilar").hidden = false;
  $("#imza").hidden = false;
  try {
    [harita, kart, rapor, imza, kanit] = await Promise.all([al("/api/harita"), al("/api/kart"), al("/api/rapor"), al("/api/imza"), al("/api/kanit")]);
  } catch (e) {
    hata(e);
    return;
  }
  harita = nesne(harita);
  kapilar = dizi(harita.endpointler).map(nesne);
  const o = nesne(harita.ozet);
  const r = nesne(rapor), g = nesne(r.girdiler), v = nesne(r.veri);
  $("#uygulama").textContent = "· " + metin(o.uygulama);
  // Kod sürümü `sezgi rapor` ile aynı metinle (git olmayan klasörde "kod sürümü yok")
  const ref = metin(r.kaynak_ref || o.kaynak_ref || "unknown");
  const kod = ref !== "unknown" ? M.kod(ref) : r.git === false ? M.kodGitYok : r.git === true ? M.kodOkunamadi : M.kodBilinmiyor;
  $("#kaynak").textContent = `${metin(o.uretici)} · ${metin(o.adaptor)} · ${kod}`;
  document.title = `Sezgi Ayna · ${metin(o.uygulama)}`;
  const ana = kapilar.filter(k => k.ana !== false);
  const S = M.sayilar;
  const sayilar = [
    [nesne(r.kapilar).ana, S.kapi],
    [ana.filter(k => k.giris === "gerekli").length, S.giris],
    [g.alan, S.alan],
    [g.kural, S.kural],
    v.bagli_kapi !== null && v.bagli_kapi !== undefined ? [v.bagli_kapi, S.bagli] : ["—", S.bagYok],
    [dizi(g.cozulemeyen_semalar).length, S.sema],
    [`${metin(nesne(o.sure_ms).toplam)} ms`, S.sure],
  ];
  yerlestir($("#sayilar"), sayilar.map(([b, s]) => h("div", { class: "sayi" }, h("b", null, b), h("span", null, s))));
  imzaCiz();
  kapsamCiz();
  const k = nesne(kart);
  yerlestir($("#kart-ozet"), kart
    ? [h("span", { class: "rozet" }, dizi(k.operations).length, M.kartIslem), h("span", { class: "rozet" }, metin(nesne(k.coverage).operations_complete), M.kartTam), h("span", { class: "rozet" }, "Benlik ", k.benlik)]
    : M.benlikYok);
  $("#kart-json").textContent = kart ? JSON.stringify(kart, null, 2) : "";
  listele();
  // Adreste bir kapı varsa (ör. başka bir ekrandan bağlantı) Kapılar sekmesi açılır
  if (location.hash.length > 1) {
    const id = decodeURIComponent(location.hash.slice(1));
    if (kapilar.some(k => k.id === id)) {
      sekmeAc("kapilar");
      sec(id);
    }
  }
})();
