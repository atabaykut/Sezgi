//! İlk temas: argümansız `sezgi` (durum ve sonraki adım), gruplu yardım, komut ayrıntısı, bilinmeyen komutta öneri.
//! Bütün komutlar tek tabloda; yardım, öneri ve README'deki komut tablosu bu tabloyla aynı gruplardır.
pub struct Komut {
    /// Türkçe asıl ad (çağrılan)
    pub ad: &'static str,
    /// İngilizce ad (aynı komuta çevrilir)
    pub en: &'static str,
    pub grup: &'static str,
    pub aciklama: &'static str,
    pub kullanim: &'static str,
    /// İngilizce açıklama ve kullanım (komut ve bayrak adları İngilizce takma adlarıyla; takma adı olmayan bayrak Türkçe)
    pub aciklama_en: &'static str,
    pub kullanim_en: &'static str,
}

impl Komut {
    /// Seçilen dildeki açıklama
    pub fn aciklama(&self) -> &'static str {
        crate::dil::sec(self.aciklama, self.aciklama_en)
    }
    /// Seçilen dildeki kullanım satırı
    pub fn kullanim(&self) -> &'static str {
        crate::dil::sec(self.kullanim, self.kullanim_en)
    }
}

const BASLA: &str = "Başla";
const BAGLA: &str = "Yapay zekâya bağla";
const BAK: &str = "Bak";
const CANLI: &str = "Canlı mod (test kopyasında ya da açıkça açılan gerçek veride iş yaptırmak)";
const GUVENLIK: &str = "Güvenlik ve veri";
const GELISMIS: &str = "Gelişmiş";

pub const KOMUTLAR: &[Komut] = &[
    Komut {
        ad: "init",
        en: "init",
        grup: BASLA,
        aciklama: "kodu okur, haritayı ve Benlik Kartı'nı .sezgi/ klasörüne yazar",
        kullanim: "sezgi init [uygulama=.] [--cikti <klasör>] [--on-yuz <klasör>] [--imzala [anahtar]] [--gecerlilik <gün>=30] [--deneme]", aciklama_en: "reads the code and writes the map and the Benlik Card to the .sezgi/ folder", kullanim_en: "sezgi init [app=.] [--out <folder>] [--frontend <folder>] [--sign [key]] [--gecerlilik <days>=30] [--dry-run]",
    },
    Komut {
        ad: "kesfet",
        en: "discover",
        grup: BASLA,
        aciklama: "kaynak kodu olmayan uygulamayı inceler: .exe/.dll/ELF kimliği, APK bildirimi, SQLite şeması; --rea ile .NET üyeleri, native ikili, firmware; onayla süreç gözlemi ve WebMCP keşfi; kanıt deposu, graf ve kart yazar",
        kullanim: "sezgi kesfet <klasör|dosya> [--cikti <klasör>] [--rea] [--imzala [anahtar]] [--json] [--onayla --yalitim konteyner|vm|yok] [--calistir --sahibiyim] [--webmcp <127.0.0.1 CDP> [--kaynak <köken>]] | sezgi kesfet --yetenekler", aciklama_en: "inspects an application without source code: .exe/.dll/ELF identity, APK manifest, SQLite schema; with --rea .NET members, native binaries, firmware; with approval process observation and WebMCP discovery; writes the evidence store, graph and card", kullanim_en: "sezgi discover <folder|file> [--out <folder>] [--rea] [--sign [key]] [--json] [--onayla --yalitim konteyner|vm|yok] [--calistir --sahibiyim] [--webmcp <127.0.0.1 CDP> [--kaynak <origin>]] | sezgi discover --yetenekler",
    },
    Komut { ad: "izle", en: "watch", grup: BASLA, aciklama: "kod değiştikçe haritayı güncel tutar", kullanim: "sezgi izle [uygulama=.] [--aralik <ms>]", aciklama_en: "keeps the map up to date as the code changes", kullanim_en: "sezgi watch [app=.] [--aralik <ms>]" },
    Komut {
        ad: "doktor",
        en: "doctor",
        grup: BASLA,
        aciklama: "bir sorun varsa neyin eksik olduğunu ve çözüm komutunu söyler; --rea ile REA köprüsünün durumu",
        kullanim: "sezgi doktor [uygulama=.] [--harita <klasör>] [--rea] [--json]", aciklama_en: "if something is wrong, says what is missing and the command that fixes it; with --rea the state of the REA bridge", kullanim_en: "sezgi doctor [app=.] [--map <folder>] [--rea] [--json]",
    },
    Komut {
        ad: "bagla",
        en: "connect",
        grup: BAGLA,
        aciklama: "yapay zekâ aracınıza bağlar; her aracı tek tek sorar, geri alınır",
        kullanim: "sezgi bagla [harita=.sezgi] [--arac claude-code,cursor,...] [--yonlendir] [--deneme]", aciklama_en: "connects to your AI tool; asks about each tool separately, can be undone", kullanim_en: "sezgi connect [map=.sezgi] [--tool claude-code,cursor,...] [--steer] [--dry-run]",
    },
    Komut {
        ad: "kaldir", en: "remove", grup: BAGLA, aciklama: "Sezgi'nin eklediği her şeyi geri alır", kullanim: "sezgi kaldir [uygulama=.] [--evet] [--harita-sil] [--deneme]", aciklama_en: "undoes everything Sezgi added", kullanim_en: "sezgi remove [app=.] [--yes] [--delete-map] [--dry-run]"
    },
    Komut {
        ad: "mcp", en: "mcp", grup: BAGLA, aciklama: "MCP sunucusu (yapay zekâ aracı bunu kendisi başlatır)", kullanim: "sezgi mcp [harita=.sezgi | --harita <klasör>] [--lang en] [--santral]", aciklama_en: "MCP server (your AI tool starts it by itself)", kullanim_en: "sezgi mcp [map=.sezgi | --map <folder>] [--lang en] [--santral]"
    },
    Komut { ad: "sun", en: "serve", grup: BAGLA, aciklama: "MCP konuşmayan araçlar için yerel HTTP + OpenAPI", kullanim: "sezgi sun [harita=.sezgi] [--port 7777]", aciklama_en: "local HTTP + OpenAPI for tools that do not speak MCP", kullanim_en: "sezgi serve [map=.sezgi] [--port 7777]" },
    Komut {
        ad: "santral",
        en: "switchboard",
        grup: BAGLA,
        aciklama: "birden fazla uygulamayı tek bağlantıda toplar",
        kullanim: "sezgi santral [ekle <harita> [--ad] [--kaynak] | ekle --istemci <klasör> --ad <ad> | cikar <ad> | baglar [--json] | bagla-varlik <uygulama:Varlik.alan> <uygulama:Varlik.alan> [--anahtar <ad>] | varliklar [--json] | varlik-gozle <A:Varlik.alan=B:Varlik.alan> [--ornek <n>] [--evet]]", aciklama_en: "brings several applications together in one connection", kullanim_en: "sezgi switchboard [add <map> [--name] [--kaynak] | add --istemci <folder> --name <name> | remove <name> | links [--json] | link-entity <app:Entity.field> <app:Entity.field> [--anahtar <name>] | entities [--json] | observe-entity <A:Entity.field=B:Entity.field> [--ornek <n>] [--yes]]",
    },
    Komut {
        ad: "ayna",
        en: "mirror",
        grup: BAK,
        aciklama: "tarayıcıda gösterir: Sezgi'nin neler yapabildiği ve bu projede neyin hazır olduğu, bu uygulama hakkında bilinenler (güven damgalı), harita, işlemler ve izinler (yalnızca bu bilgisayar; anahtarlı adresle açılır; harita yoksa tanıtım ve ilk adım); `--adres` çalışan Ayna'nın adresini yeniden yazar",
        kullanim: "sezgi ayna [harita=.sezgi] [--port 4777] | sezgi ayna --adres", aciklama_en: "shows in your browser what Sezgi can do and what is ready in this project, what is known about this application (with trust stamps), the map, operations and permissions (this computer only; opens with a keyed address; without a map, an introduction and the first step); `--adres` prints the address of the running Mirror again", kullanim_en: "sezgi mirror [map=.sezgi] [--port 4777] | sezgi mirror --adres",
    },
    Komut { ad: "rapor", en: "report", grup: BAK, aciklama: "neyin bilindiğini, neyin bilinmediğini listeler", kullanim: "sezgi rapor [harita=.sezgi] [--json]", aciklama_en: "lists what is known and what is not known", kullanim_en: "sezgi report [map=.sezgi] [--json]" },
    Komut {
        ad: "karsilastir",
        en: "diff",
        grup: BAK,
        aciklama: "iki sürüm arasında eklenen, kaldırılan ve bozucu değişiklikler",
        kullanim: "sezgi karsilastir <eski-harita> <yeni-harita> [--json] [--sert]", aciklama_en: "added, removed and breaking changes between two versions", kullanim_en: "sezgi diff <old-map> <new-map> [--json] [--strict]",
    },
    Komut {
        ad: "olc",
        en: "measure",
        grup: BAK,
        aciklama: "kendi uygulamanızda ölçüm: hız ve (varsa) OpenAPI'ye göre doğruluk",
        kullanim: "sezgi olc [uygulama=.] [--openapi <dosya>] [--onek /api] [--json]", aciklama_en: "measures on your own application: speed and (if available) accuracy against OpenAPI", kullanim_en: "sezgi measure [app=.] [--openapi <file>] [--onek /api] [--json]",
    },
    Komut {
        ad: "canli",
        en: "live",
        grup: CANLI,
        aciklama: "durum; `canli kur` hedef, mod, oturum ve ilk token'ı sırayla kurar; `canli tasi` eski sürümün uygulama adıyla tuttuğu durumu bu projeye taşır",
        kullanim: "sezgi canli | sezgi canli kur [--hedef <adres>] [--test-kopyasi] [--mod okuma|yazma] [--oidc <issuer> --istemci <id>] [--izin <desen:seviye>]... [--token-ad <ad>] [--tum-islemler] | sezgi canli tasi", aciklama_en: "status; `live setup` sets up target, mode, session and the first token in order; `live migrate` moves state an older version kept under the app name to this project", kullanim_en: "sezgi live | sezgi live setup [--hedef <address>] [--test-kopyasi] [--mod okuma|yazma] [--oidc <issuer> --istemci <id>] [--izin <pattern:level>]... [--token-ad <name>] [--tum-islemler] | sezgi live migrate",
    },
    Komut { ad: "mod", en: "mode", grup: CANLI, aciklama: "gözlem (varsayılan), okuma ya da yazma", kullanim: "sezgi mod [gozlem|okuma|yazma]", aciklama_en: "observe (default), read or write", kullanim_en: "sezgi mode [observe|read|write]" },
    Komut {
        ad: "hedef",
        en: "target",
        grup: CANLI,
        aciklama: "isteklerin gideceği adres; yazma test kopyasına, ya da insanın yalnızca terminalde uygulamanın adını yazarak açtığı gerçek veriye (deneysel: yöneticinin kurduğu sistem çapası ve insan parolası ister, her yazma terminalde parolayla onaylanır; saatlik yazma sınırı, varsayılan 20; aynı kullanıcı hesabında kabuk erişimi olan ajan parolayı bilmeden açamaz, parolayı öğrenen ya da yönetici yetkisiyle çalışan programı durdurmaz: sezgi insan)",
        kullanim: "sezgi hedef <http(s)://...> [--test-kopyasi | --gercek-veri] [--saatlik-yazma <1-200>] | hedef --saatlik-yazma <1-200>", aciklama_en: "where requests go; writes go to a test copy, or to real data a human opened by typing the app's name in a terminal (experimental: needs the system anchor set up by an administrator and the human password, every write is approved with the password in a terminal; hourly write limit, default 20; an agent with shell access in the same user account cannot open it without the password, but it does not stop a program that learned the password or runs with administrator rights: sezgi human)", kullanim_en: "sezgi target <http(s)://...> [--test-kopyasi | --gercek-veri] [--saatlik-yazma <1-200>] | target --saatlik-yazma <1-200>",
    },
    Komut {
        ad: "oturum",
        en: "session",
        grup: CANLI,
        aciklama: "insanın uygulamadaki kendi oturumu (elle ya da kurumsal girişle) ve doğrulanmış rolleri (terminalde açık beyan, imzalı kimlik belirteci ya da uygulamanın \"ben\" ucu; terminalsiz --rol beyanı doğrulanmış sayılmaz; sistem çapası kuruluysa roller insan parolasıyla imzalanınca sayılır); elle girişte oturum anahtarı terminalde sorulur, terminal yoksa stdin'den tek satır okunur",
        kullanim: "sezgi oturum ac [--sure <saat>] [--rol <rol>]... [--rol-ucu <yol> [--rol-yolu <json.yolu>]] [--oidc <issuer> --istemci <id> [--rol-claim <yol>]...] | oturum kapat", aciklama_en: "the human's own session in the application (by hand or with company sign-in) and verified roles (an explicit statement in a terminal, a signed identity token or the app's \"me\" endpoint; a --rol statement without a terminal is not verified; with the system anchor set up, roles count once signed with the human password); for a manual sign-in the session key is asked in the terminal, without a terminal one line is read from stdin", kullanim_en: "sezgi session open [--sure <hours>] [--rol <role>]... [--rol-ucu <path> [--rol-yolu <json.path>]] [--oidc <issuer> --istemci <id> [--rol-claim <path>]...] | session close",
    },
    Komut {
        ad: "token",
        en: "token",
        grup: CANLI,
        aciklama: "yapay zekânın hangi işlemleri yapabileceği (varsayılan kapalı; `*` deseniyle yazma açık onay ister: --tum-islemler; hiçbir işleme uymayan desen uyarılır, --kesin ile hata; gerçek veride yazan token yalnızca terminalde --gercek-veri ve insan parolasıyla)",
        kullanim: "sezgi token olustur --ad <ad> --izin <desen:oku|yaz|yikici>... [--sure <gün>] [--onay yikici|hepsi] [--tum-islemler] [--kesin] [--gercek-veri] | token listele | token iptal <ad>", aciklama_en: "which operations the AI may perform (off by default; writing with the `*` pattern needs explicit approval: --tum-islemler; a pattern that matches no operation is warned about, an error with --kesin; a token that writes to real data only in a terminal with --gercek-veri and the human password)", kullanim_en: "sezgi token create --name <name> --izin <pattern:oku|yaz|yikici>... [--sure <days>] [--onay yikici|hepsi] [--tum-islemler] [--kesin] [--gercek-veri] | token list | token revoke <name>",
    },
    Komut {
        ad: "onay",
        en: "approve",
        grup: CANLI,
        aciklama: "insan onayı bekleyen istekler; `onay goster` gövdeyi ve özetini gösterir, her istek bu özetle onaylanır (gerçek veride yalnızca terminalde, --ozet olmadan, insan parolasıyla)",
        kullanim: "sezgi onay listele | onay goster <id> | onay ver <id> [--ozet <özetin ilk 8 karakteri>] | onay reddet <id>", aciklama_en: "requests waiting for human approval; `approve show` shows the body and its digest, each request is approved with that digest (on real data only in a terminal, without --ozet, with the human password)", kullanim_en: "sezgi approve list | approve show <id> | approve give <id> [--ozet <first 8 characters of the digest>] | approve reject <id>",
    },
    Komut {
        ad: "plan",
        en: "plan",
        grup: CANLI,
        aciklama: "yapay zekânın hazırladığı çok uygulamalı değişiklik planı: gövdeleri görür, adımları sırayla onaylarsınız (plan kendiliğinden yürümez)",
        kullanim: "sezgi plan listele | plan goster <id> | plan onayla <id> [--adim <n>] [--ozet <özetin ilk 8 karakteri>] | plan reddet <id>", aciklama_en: "a change plan across several apps prepared by the AI: you see the bodies and approve the steps in order (a plan never runs by itself)", kullanim_en: "sezgi plan list | plan show <id> | plan approve <id> [--adim <n>] [--ozet <first 8 characters of the digest>] | plan reject <id>",
    },
    Komut { ad: "kayit", en: "log", grup: CANLI, aciklama: "şifreli, zincirli işlem kaydı", kullanim: "sezgi kayit [--dogrula] [--json] [--son <n>]", aciklama_en: "encrypted, chained operation log", kullanim_en: "sezgi log [--dogrula] [--json] [--son <n>]" },
    Komut {
        ad: "gozlem",
        en: "observe",
        grup: BAK,
        aciklama: "tarayıcı ağ kaydını (HAR) ya da onaylı bir tarayıcı senaryosunu (REA; UI adımı → istek) gözlem kanıtı olarak alır: yalnızca biçim (yöntem, yol şablonu, durum, alan adları); değer, başlık, çerez, belirteç saklanmaz",
        kullanim: "sezgi gozlem al <dosya.har> [--harita <klasör>] [--isaretler <json>] | sezgi gozlem senaryo <senaryo.json> --cdp <127.0.0.1 CDP> [--harita <klasör>] --onayla --yalitim konteyner|vm|yok --calistir --sahibiyim", aciklama_en: "takes a browser network capture (HAR) or an approved browser scenario (REA; UI step → request) as observation evidence: shape only (method, path template, status, field names); no values, headers, cookies or tokens are kept", kullanim_en: "sezgi observe import <file.har> [--map <folder>] [--isaretler <json>] | sezgi observe scenario <scenario.json> --cdp <127.0.0.1 CDP> [--map <folder>] --onayla --yalitim konteyner|vm|yok --calistir --sahibiyim",
    },
    Komut {
        ad: "anahtar",
        en: "key",
        grup: GUVENLIK,
        aciklama: "Benlik Kartı imza anahtarı oluşturur ya da bir uygulamanın anahtarını sabitler",
        kullanim: "sezgi anahtar olustur [--ad <anahtar>] | anahtar sabitle <benlik.json> <benlik-keys.json>", aciklama_en: "creates a Benlik Card signing key or pins an application's key", kullanim_en: "sezgi key create [--name <key>] | key pin <benlik.json> <benlik-keys.json>",
    },
    Komut {
        ad: "insan",
        en: "human",
        grup: GUVENLIK,
        aciklama: "gerçek veride insanın kanıtı: yöneticinin kurduğu sistem çapası ve yalnızca insanın bildiği parola; gerçek veride açma, gerçek-veri token'ı, her onay ve sınır değişikliği bu parolayı ister (aynı hesaptaki ajan çapayı değiştiremez)",
        kullanim: "sezgi insan [durum] | sudo sezgi insan kur --sistem [--kullanici <uid|ad>]", aciklama_en: "human proof on real data: the system anchor set up by an administrator and a password only the human knows; opening real data, real-data tokens, every approval and limit change ask for this password (an agent in the same account cannot change the anchor)", kullanim_en: "sezgi human [status] | sudo sezgi human setup --system [--user <uid|name>]",
    },
    Komut { ad: "dogrula", en: "verify", grup: GUVENLIK, aciklama: "kartın imzasını, geçerlilik süresini ve kanıt paketini doğrular", kullanim: "sezgi dogrula <benlik.json> <benlik-keys.json>", aciklama_en: "verifies the card's signature, validity period and evidence bundle", kullanim_en: "sezgi verify <benlik.json> <benlik-keys.json>" },
    Komut { ad: "defter", en: "ledger", grup: GUVENLIK, aciklama: "Sezgi'nin bu uygulama için ne tuttuğu ve saklama süresi", kullanim: "sezgi defter [--saklama <gün>]", aciklama_en: "what Sezgi keeps for this application and for how long", kullanim_en: "sezgi ledger [--saklama <days>]" },
    Komut {
        ad: "unut",
        en: "forget",
        grup: GUVENLIK,
        aciklama: "bir kişinin ya da bütün müşteri verisinin silinmesi (KVKK / GDPR)",
        kullanim: "sezgi unut --kisi <kimlik> | sezgi unut --hepsi [--evet]", aciklama_en: "deletes one person's data or all customer data (KVKK / GDPR)", kullanim_en: "sezgi forget --person <id> | sezgi forget --all [--yes]",
    },
    Komut {
        ad: "hafiza",
        en: "memory",
        grup: GELISMIS,
        aciklama: "aynı okumanın ikinci kez uygulamaya gitmemesi (varsayılan kapalı); oturumla alınan cevaplar yalnızca --oturumlu ile",
        kullanim: "sezgi hafiza [ac [--sure <gün>] [--sinir <MB>] [--oturumlu] | kapat | temizle]", aciklama_en: "keeps the same read from going to the application twice (off by default); answers taken with a session only with --oturumlu", kullanim_en: "sezgi memory [on [--sure <days>] [--sinir <MB>] [--oturumlu] | off | clear]",
    },
    Komut {
        ad: "paylas",
        en: "share",
        grup: GELISMIS,
        aciklama: "ekibin depoya ekleyeceği imzalı, sıkıştırılmış harita",
        kullanim: "sezgi paylas [uygulama=.] [--imzala <anahtar>]", aciklama_en: "a signed, compressed map your team adds to the repository", kullanim_en: "sezgi share [app=.] [--sign <key>]",
    },
    Komut {
        ad: "rea",
        en: "rea",
        grup: GELISMIS,
        aciklama: "isteğe bağlı REA'yı (kaynaksız .NET, native ve firmware analizi) sabitlenmiş sürümle Sezgi'nin klasörüne kurar, durumunu gösterir, kaldırır; kurulum terminalde \"evet\" ister",
        kullanim: "sezgi rea kur | sezgi rea durum [--json] | sezgi rea kaldir [--evet]", aciklama_en: "installs the optional REA (.NET, native and firmware analysis without source) with a pinned version into Sezgi's folder, shows its state, removes it; installing asks for \"yes\" in a terminal", kullanim_en: "sezgi rea install | sezgi rea status [--json] | sezgi rea uninstall [--yes]",
    },
    Komut { ad: "teshis", en: "diagnostics", grup: GELISMIS, aciklama: "yalnızca sayaç tutan yerel tanılama (hiçbir yere gönderilmez)", kullanim: "sezgi teshis [ac|kapat]", aciklama_en: "local diagnostics that keep counters only (never sent anywhere)", kullanim_en: "sezgi diagnostics [on|off]" },
];

const GRUPLAR: [&str; 6] = [BASLA, BAGLA, BAK, CANLI, GUVENLIK, GELISMIS];

/// Ad (Türkçe ya da İngilizce) -> komut
pub fn bul(ad: &str) -> Option<&'static Komut> {
    KOMUTLAR.iter().find(|k| k.ad == ad || k.en == ad)
}

fn surum() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Argümansız `sezgi`: harita yoksa nasıl başlanacağı ve Sezgi'yi tanıtan Ayna; varsa durum ve sonraki adım (en fazla 12 satır)
pub fn ana_ekran() {
    let kok = std::env::current_dir().unwrap_or_default();
    let kapilar = kok.join(".sezgi").join("kapilar.json");
    let Some(h) = std::fs::read_to_string(&kapilar).ok().and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok()) else {
        tp!("Sezgi {}: uygulamanızın kodunu okuyup yapay zekâya tanıtır.", "Sezgi {}: reads your application's code and introduces it to AI.", surum());
        println!();
        tp!("Neler yapabildiğini tarayıcıda görün (harita gerekmez):", "See what it can do in your browser (no map needed):");
        tp!("  sezgi ayna     yetenekler, bu klasörün durumu ve sıradaki adım", "  sezgi mirror   capabilities, the state of this folder and the next step");
        println!();
        tp!("Başlamak için uygulamanızın klasöründe:", "To start, in your application's folder:");
        tp!("  sezgi init     kodu okur, haritayı çıkarır (birkaç saniye)", "  sezgi init     reads the code and builds the map (a few seconds)");
        tp!("  sezgi bagla    yapay zekâ aracınıza bağlar", "  sezgi connect  connects it to your AI tool");
        println!();
        tp!("Bütün komutlar: sezgi yardim", "All commands: sezgi help");
        return;
    };
    let uygulama = h["ozet"]["uygulama"].as_str().unwrap_or(t!("uygulama", "application"));
    let n = h["endpointler"].as_array().map(|a| a.len()).unwrap_or(0);
    tp!("Sezgi {} · {uygulama} · {n} işlem", "Sezgi {} · {uygulama} · {n} operations", surum());
    let degisen = crate::izle::degisen_dosyalar(&kok, &kok.join(".sezgi")).len();
    let mut sonraki: Vec<(&str, &str)> = vec![];
    if degisen > 0 {
        tp!("  Harita     .sezgi · {degisen} dosya haritadan sonra değişti", "  Map        .sezgi · {degisen} files changed since the map was built");
        sonraki.push(("sezgi init", t!("haritayı yenile (ya da sürekli: sezgi izle)", "refresh the map (or continuously: sezgi watch)")));
    } else {
        tp!("  Harita     .sezgi · güncel", "  Map        .sezgi · up to date");
    }
    let baglantilar = crate::baglan::baglanti_durumlari(&kok);
    if baglantilar.is_empty() {
        tp!("  Bağlantı   yok", "  Connection none");
        sonraki.push((t!("sezgi bagla", "sezgi connect"), t!("yapay zekâ aracınıza bağla", "connect it to your AI tool")));
    } else {
        let adlar: Vec<&str> = baglantilar.iter().map(|(a, _, _)| a.as_str()).collect();
        tp!("  Bağlantı   {}", "  Connection {}", adlar.join(", "));
    }
    sonraki.push((
        t!("sezgi ayna", "sezgi mirror"),
        t!("neler yapabildiğini ve bu uygulama hakkında bilinenleri tarayıcıda gör", "see what it can do and what is known about this application in your browser"),
    ));
    println!();
    tp!("Sonraki adım:", "Next step:");
    for (k, a) in sonraki.iter().take(3) {
        if crate::dil::en() {
            println!("  {k:<15}{a}");
        } else {
            println!("  {k:<13}{a}");
        }
    }
    tp!("Bütün komutlar: sezgi yardim", "All commands: sezgi help");
}

/// `sezgi yardim [komut]`
pub fn yardim(args: &[String]) {
    if let Some(ad) = args.get(1) {
        match bul(ad) {
            Some(k) => {
                if crate::dil::en() {
                    println!("{}  ({} · Turkish name: {})", k.kullanim(), grup_adi(k.grup), k.ad);
                } else {
                    println!("{}  ({} · İngilizce: {})", k.kullanim(), k.grup, k.en);
                }
                println!("  {}", ilk_buyuk(k.aciklama()));
            }
            None => bilinmeyen(ad),
        }
        return;
    }
    tp!("Sezgi {}: uygulamanızın kodunu okuyup yapay zekâya tanıtır.", "Sezgi {}: reads your application's code and introduces it to AI.", surum());
    for g in GRUPLAR {
        println!();
        println!("{}", grup_adi(g));
        for k in KOMUTLAR.iter().filter(|k| k.grup == g) {
            // İngilizce çıktıda önce İngilizce ad; iki ad da çağrılabilir
            let adlar = if k.ad == k.en {
                k.ad.to_string()
            } else if crate::dil::en() {
                format!("{} ({})", k.en, k.ad)
            } else {
                format!("{} ({})", k.ad, k.en)
            };
            println!("  {adlar:<26}{}", k.aciklama());
        }
    }
    println!();
    tp!("Ayrıntı: sezgi yardim <komut>   ·   Sürüm: sezgi --version", "Details: sezgi help <command>   ·   Version: sezgi --version");
    tp!("Dil: --lang en|tr (her komutta) ya da SEZGI_LANG; verilmezse sistem dili", "Language: --lang en|tr (any command) or SEZGI_LANG; otherwise the system language");
}

/// Grup başlığının seçilen dildeki adı
fn grup_adi(g: &str) -> &str {
    match g {
        BASLA => crate::dil::sec(BASLA, "Start"),
        BAGLA => crate::dil::sec(BAGLA, "Connect to AI"),
        BAK => crate::dil::sec(BAK, "Look"),
        CANLI => crate::dil::sec(CANLI, "Live mode (have work done on a test copy, or on real data opened explicitly)"),
        GUVENLIK => crate::dil::sec(GUVENLIK, "Security and data"),
        GELISMIS => crate::dil::sec(GELISMIS, "Advanced"),
        x => x,
    }
}

/// İlk harf büyük, Türkçe kurala göre: `i` → `İ` (Unicode `to_uppercase` noktasız `I` verir)
fn ilk_buyuk(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some('i') if !crate::dil::en() => format!("İ{}", c.as_str()),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Bayrağın aldığı değer
#[derive(Clone, Copy, PartialEq)]
enum Deger {
    /// Değer almaz
    Yok,
    /// Bir değer ister
    Metin,
    /// Negatif olmayan tamsayı ister
    Sayi,
    /// 0–65535
    Port,
    /// Değeri isteğe bağlı (`--imzala [anahtar]`)
    Secimli,
}

use Deger::{Metin, Port, Sayi, Secimli, Yok};

const INIT_BAYRAKLARI: &[(&str, Deger)] = &[("--cikti", Metin), ("--on-yuz", Metin), ("--imzala", Secimli), ("--gecerlilik", Sayi), ("--deneme", Yok), ("--paylasimsiz", Yok)];
const HARITA: (&str, Deger) = ("--harita", Metin);
/// Bütün komutlarda: çıktı dili (`main` bayrağı `dil::kur` ile komut satırından çıkarır; burada izinli listede durur)
const DIL: (&str, Deger) = ("--lang", Metin);

/// Komutun (iç adıyla) kabul ettiği bayraklar. Burada olmayan bayrak hata verir: yazım hatası sessizce yok sayılmaz
fn bayraklar(komut: &str) -> Option<Vec<(&'static str, Deger)>> {
    let mut v: Vec<(&str, Deger)> = match komut {
        "init" => INIT_BAYRAKLARI.to_vec(),
        "calistir" => [INIT_BAYRAKLARI, &[("--aralik", Sayi), ("--en-fazla", Sayi)]].concat(),
        "add" => vec![("--proje", Metin), ("--arac", Metin), ("--yonlendir", Yok), ("--deneme", Yok), ("--elle", Yok)],
        "kaldir" => vec![("--harita-sil", Yok), ("--deneme", Yok), ("--evet", Yok)],
        "doktor" => vec![HARITA, ("--json", Yok), ("--rea", Yok)],
        "olc" => vec![HARITA, ("--json", Yok), ("--onek", Metin), ("--openapi", Metin)],
        "paylas" => vec![("--imzala", Secimli)],
        "karsilastir" => vec![("--json", Yok), ("--sert", Yok)],
        "kesfet" => vec![
            ("--cikti", Metin),
            ("--rea", Yok),
            ("--imzala", Secimli),
            ("--json", Yok),
            ("--yetenekler", Yok),
            ("--onayla", Yok),
            ("--yalitim", Metin),
            ("--calistir", Yok),
            ("--sahibiyim", Yok),
            ("--webmcp", Metin),
            ("--kaynak", Metin),
        ],
        "mod" => vec![HARITA],
        // `onay goster` / `onay ver --ozet`: onaydan önce gövdenin gösterilmesi ve özetle bağlanması
        "onay" => vec![HARITA, ("--ozet", Metin)],
        "hedef" => vec![HARITA, ("--test-kopyasi", Yok), ("--gercek-veri", Yok), ("--saatlik-yazma", Sayi)],
        "token" => vec![HARITA, ("--ad", Metin), ("--izin", Metin), ("--sure", Sayi), ("--onay", Metin), ("--tum-islemler", Yok), ("--kesin", Yok), ("--gercek-veri", Yok)],
        "oturum" => vec![
            HARITA,
            ("--sure", Sayi),
            ("--oidc", Metin),
            ("--istemci", Metin),
            ("--kapsam", Metin),
            ("--hedef-kitle", Metin),
            ("--dar-kapsam", Metin),
            ("--rol", Metin),
            ("--rol-ucu", Metin),
            ("--rol-yolu", Metin),
            ("--rol-claim", Metin),
        ],
        "canli" => vec![
            HARITA,
            ("--hedef", Metin),
            ("--test-kopyasi", Yok),
            ("--mod", Metin),
            ("--oidc", Metin),
            ("--istemci", Metin),
            ("--izin", Metin),
            ("--token-ad", Metin),
            ("--tum-islemler", Yok),
        ],
        "kayit" => vec![HARITA, ("--dogrula", Yok), ("--json", Yok), ("--son", Sayi)],
        "gozlem" => vec![HARITA, ("--isaretler", Metin), ("--cdp", Metin), ("--onayla", Yok), ("--yalitim", Metin), ("--calistir", Yok), ("--sahibiyim", Yok)],
        "defter" => vec![HARITA, ("--saklama", Sayi)],
        "unut" => vec![("--kisi", Metin), ("--hepsi", Yok), ("--evet", Yok)],
        "teshis" | "dogrula" => vec![],
        "santral" => vec![("--ad", Metin), ("--kaynak", Metin), ("--istemci", Metin), ("--json", Yok), ("--anahtar", Metin), ("--ornek", Sayi), ("--evet", Yok)],
        "plan" => vec![("--adim", Sayi), ("--ozet", Metin)],
        "hafiza" => vec![HARITA, ("--sure", Sayi), ("--sinir", Sayi), ("--oturumlu", Yok)],
        "sun" => vec![("--port", Port)],
        "mcp" => vec![HARITA, ("--santral", Yok)],
        "anahtar" => vec![("--ad", Metin)],
        "insan" => vec![("--sistem", Yok), ("--kullanici", Metin)],
        "ayna" => vec![("--port", Port), ("--adres", Yok)],
        "rapor" => vec![("--json", Yok)],
        "rea" => vec![("--json", Yok), ("--evet", Yok)],
        _ => return None,
    };
    v.push(DIL);
    Some(v)
}

/// Alt komutu olan komutlar: (Türkçe alt komutlar, İngilizce karşılıklar, alt komut zorunlu mu)
type AltKomutlar = (&'static [&'static str], &'static [(&'static str, &'static str)], bool);

fn alt_komutlar(komut: &str) -> Option<AltKomutlar> {
    Some(match komut {
        "hafiza" => (&["ac", "kapat", "temizle"], &[("on", "ac"), ("enable", "ac"), ("off", "kapat"), ("disable", "kapat"), ("clear", "temizle")], false),
        "teshis" => (&["ac", "kapat"], &[("on", "ac"), ("enable", "ac"), ("off", "kapat"), ("disable", "kapat")], false),
        "santral" => (
            &["ekle", "cikar", "baglar", "bagla-varlik", "varliklar", "varlik-gozle"],
            &[("add", "ekle"), ("remove", "cikar"), ("links", "baglar"), ("link-entity", "bagla-varlik"), ("entities", "varliklar"), ("observe-entity", "varlik-gozle")],
            false,
        ),
        "plan" => (&["listele", "goster", "onayla", "reddet"], &[("list", "listele"), ("show", "goster"), ("approve", "onayla"), ("reject", "reddet")], false),
        "token" => (&["olustur", "listele", "iptal"], &[("create", "olustur"), ("list", "listele"), ("revoke", "iptal")], false),
        "onay" => (&["listele", "goster", "ver", "reddet"], &[("list", "listele"), ("approve", "ver"), ("give", "ver"), ("reject", "reddet"), ("show", "goster")], false),
        "oturum" => (&["ac", "kapat"], &[("open", "ac"), ("close", "kapat")], true),
        "canli" => (&["kur", "tasi"], &[("setup", "kur"), ("migrate", "tasi")], false),
        "gozlem" => (&["al", "senaryo"], &[("import", "al"), ("scenario", "senaryo")], true),
        "insan" => (&["durum", "kur"], &[("status", "durum"), ("setup", "kur"), ("install", "kur")], false),
        "anahtar" => (&["olustur", "sabitle"], &[("create", "olustur"), ("pin", "sabitle")], true),
        "rea" => (&["kur", "durum", "kaldir"], &[("install", "kur"), ("setup", "kur"), ("status", "durum"), ("uninstall", "kaldir"), ("remove", "kaldir")], true),
        "mod" => (&["gozlem", "okuma", "yazma"], &[("observe", "gozlem"), ("read", "okuma"), ("write", "yazma")], false),
        _ => return None,
    })
}

/// İç komut adı -> yardım tablosundaki adı
fn gorunen_ad(komut: &str) -> &str {
    match komut {
        "calistir" => "izle",
        "add" => "bagla",
        x => x,
    }
}

/// Komut satırının ilk denetimi (komut çalışmadan, hiçbir dosyaya dokunmadan):
/// - `--help` / `-h` herhangi bir konumda, ya da alt komut yerinde `help` / `yardim`: yalnızca o komutun yardımı
/// - İngilizce alt komutlar Türkçe karşılığına çevrilir; bilinmeyen alt komut hata verir
/// - Bilinmeyen bayrak, değersiz ya da geçersiz değerli bayrak hata verir (çıkış kodu 2)
pub fn denetle(args: &mut [String]) {
    let Some(komut) = args.first().cloned() else { return };
    if matches!(komut.as_str(), "yardim" | "--version" | "-v") {
        return;
    }
    let ad = gorunen_ad(&komut).to_string();
    let Some(k) = bul(&ad) else { return };
    let yardim_istendi = args.iter().any(|a| a == "--help" || a == "-h") || args.get(1).is_some_and(|a| a == "help" || a == "yardim");
    if yardim_istendi {
        yardim(&["yardim".to_string(), ad]);
        std::process::exit(0);
    }
    // `add mcp`: alt komut konumu "mcp"; bayraklar ondan sonra
    let ilk_bayrak = if komut == "add" { 2 } else { 1 };
    if let Some((turkce, ingilizce, zorunlu)) = alt_komutlar(&komut) {
        match args.get(1).filter(|a| !a.starts_with("--")) {
            Some(a) => {
                if let Some((_, t)) = ingilizce.iter().find(|(e, _)| e == a) {
                    args[1] = t.to_string();
                } else if !turkce.contains(&a.as_str()) {
                    if crate::dil::en() {
                        let secenekler: Vec<&str> = turkce.iter().map(|t| ingilizce.iter().find(|(_, k)| k == t).map_or(*t, |(e, _)| *e)).collect();
                        eprintln!("✗ Unknown subcommand: sezgi {ad} {a}. Options: {}", secenekler.join(", "));
                    } else {
                        eprintln!("✗ Bilinmeyen alt komut: sezgi {ad} {a}. Seçenekler: {}", turkce.join(", "));
                    }
                    te!("  Kullanım: {}", "  Usage: {}", k.kullanim());
                    std::process::exit(2);
                }
            }
            None if zorunlu => eksik(&ad),
            None => {}
        }
    }
    let Some(izinli) = bayraklar(&komut) else { return };
    let hata = |m: String| -> ! {
        eprintln!("✗ {m}");
        te!("  Kullanım: {}", "  Usage: {}", k.kullanim());
        std::process::exit(2)
    };
    let mut i = ilk_bayrak;
    while i < args.len() {
        let a = &args[i];
        if !a.starts_with("--") {
            i += 1;
            continue;
        }
        let Some((_, tur)) = izinli.iter().find(|(b, _)| b == a) else {
            let oneri = izinli.iter().map(|(b, _)| (*b, uzaklik(a, b))).filter(|(_, u)| *u <= 3).min_by_key(|(_, u)| *u).map(|(b, _)| b);
            match oneri {
                Some(b) => hata(tf!("Bilinmeyen bayrak: {a} (sezgi {ad}). Bunu mu demek istediniz: {b}", "Unknown flag: {a} (sezgi {ad}). Did you mean: {b}")),
                None => hata(tf!("Bilinmeyen bayrak: {a} (sezgi {ad})", "Unknown flag: {a} (sezgi {ad})")),
            }
        };
        let deger = args.get(i + 1).filter(|d| !d.starts_with("--"));
        match (tur, deger) {
            (Yok, _) => {}
            (Secimli, Some(_)) => i += 1,
            (Secimli, None) => {}
            (_, None) => hata(tf!("{a} bir değer ister", "{a} needs a value")),
            (Sayi, Some(d)) if d.parse::<u64>().is_err() => hata(tf!("{a} negatif olmayan bir tamsayı ister: {d}", "{a} needs a non-negative integer: {d}")),
            (Port, Some(d)) if d.parse::<u16>().is_err() => hata(tf!("{a} 0–65535 arasında bir port ister: {d}", "{a} needs a port between 0 and 65535: {d}")),
            (_, Some(_)) => i += 1,
        }
        i += 1;
    }
}

/// Bilinmeyen komut: en yakın komut önerilir; çıkış kodu 2
pub fn bilinmeyen(ad: &str) -> ! {
    te!("Bilinmeyen komut: {ad}.", "Unknown command: {ad}.");
    if let Some(oneri) = en_yakin(ad) {
        te!("Bunu mu demek istediniz: sezgi {oneri}", "Did you mean: sezgi {oneri}");
    }
    te!("Bütün komutlar: sezgi yardim", "All commands: sezgi help");
    std::process::exit(2);
}

/// Kullanım hatası (eksik ya da geçersiz argüman, değer, bayrak): ne yanlış ve komutun kullanım satırı; çıkış kodu 2.
/// Çalışma hatası (dosya yok, durum uygun değil, uygulamaya ulaşılamadı) 1 ile çıkar
pub fn kullanim_hatasi(komut: &str, mesaj: &str) -> ! {
    eprintln!("✗ {mesaj}");
    if let Some(k) = bul(gorunen_ad(komut)) {
        te!("  Kullanım: {}", "  Usage: {}", k.kullanim());
    }
    std::process::exit(2)
}

/// Eksik argüman: komutun kullanım satırı; çıkış kodu 2
pub fn eksik(ad: &str) -> ! {
    match bul(ad) {
        Some(k) => te!("Eksik argüman. Kullanım: {}", "Missing argument. Usage: {}", k.kullanim()),
        None => te!("Eksik argüman. Bütün komutlar: sezgi yardim", "Missing argument. All commands: sezgi help"),
    }
    std::process::exit(2);
}

fn en_yakin(ad: &str) -> Option<&'static str> {
    let ad = ad.to_lowercase();
    KOMUTLAR
        .iter()
        .flat_map(|k| [k.ad, k.en])
        .map(|a| (a, uzaklik(&ad, a)))
        // Eşik kısa olana göre: üç harflik bir yazım hatası en çok bir harf uzakta olabilir ("foo" → "mod" önerilmez)
        .filter(|(a, u)| *u <= 1.max(ad.chars().count().min(a.chars().count()) / 3) || a.starts_with(&ad))
        .min_by_key(|(_, u)| *u)
        .map(|(a, _)| a)
}

/// Levenshtein uzaklığı
pub(crate) fn uzaklik(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut onceki: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut simdi = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            simdi.push((onceki[j] + usize::from(ca != *cb)).min(onceki[j + 1] + 1).min(simdi[j] + 1));
        }
        onceki = simdi;
    }
    onceki[b.len()]
}

/// Giriş kararının insan dilindeki karşılığı (JSON'daki değer değişmez)
pub fn giris_metni(g: &str) -> &str {
    match g {
        "gerekli" => t!("giriş gerekli", "login required"),
        "herkese_acik" => t!("girişsiz", "no login"),
        "istege_bagli" => t!("giriş isteğe bağlı", "login optional"),
        x => x,
    }
}
