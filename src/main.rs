//! Sezgi komut satırı (0.1): `init`, `mcp`, `rapor`, `ayna`, `anahtar olustur`, `dogrula` (İngilizce takma adlarıyla).
//!
//! `init` uygulamayı paralel olarak okur, çatı adaptörüyle kapıları, girdileri, kuralları ve tablo bağlarını çıkarır,
//! `kapilar.json` ve `benlik.json` dosyalarını yazar, kapsam özetini ekrana basar.
//! Uygulama koduna hiçbir şey yazmaz; `--cikti` verilmezse `<uygulama>/.sezgi` kullanılır.

// Çıktı dili ve çeviri makroları (`t!`, `tf!`, `tp!`, `te!`): öteki modüllerden önce tanımlanır
#[macro_use]
mod dil;
mod adaptor;
mod anlam;
mod kapsam;
// ASP.NET Core (C#, kaynak kodlu)
mod aspnet;
// Kaynaksız keşif (APK bildirimi): komuta bağlanması kaynaksız keşif komutuyla birlikte yapılır
mod android;
mod ayna;
mod baglan;
mod benlik;
mod cagri;
mod calisma_alani;
mod canli;
// Dış etki (giden çağrılar), geri alınabilirlik ve iş durumu ön koşulları
mod dis;
mod dis_zincir;
mod django;
mod dna;
mod doktor;
mod express;
mod fark;
mod fastapi;
mod flask;
mod gercek_veri;
mod gohttp;
mod golang;
mod gozlem;
mod graf;
mod guvenli;
mod hafiza;
mod http;
mod imza;
mod insan;
mod islem_arac;
mod izle;
mod kanit;
mod kesif;
mod kotlin;
mod ktor;
mod kural;
mod laravel;
mod mcp;
mod model;
mod oidc;
mod olc;
mod openapi;
mod paylas;
mod plan;
mod py_ortak;
mod python;
mod rails;
mod rapor;
mod santral;
mod senaryo;
mod spring;
mod sun;
mod tanitim;
mod teshis;
mod varlik;
mod web;
mod yardim;
mod yolculuk;

use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const NESTJS: &str = include_str!("../adapters/nestjs.toml");
use guvenli::EN_BUYUK_DOSYA;
pub(crate) use kesif::{adaptor_kimligi, cati, cati_sec, parmak, Onbellek, Parmak, Secim};

/// Uygulamayı içeren git deposunun kökü
fn git_koku(kok: &Path) -> Option<PathBuf> {
    kok.ancestors().find(|k| k.join(".git").exists()).map(Path::to_path_buf)
}

/// Metnin başı bir commit özetiyse (40 ya da 64 onaltılık) ilk 12 karakteri; değilse "unknown"
/// (dosyadaki rastgele metin karta commit diye yazılmasın)
fn commit_kisa(s: &str) -> String {
    let ozet: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if ozet.len() == 40 || ozet.len() == 64 {
        ozet.chars().take(12).collect()
    } else {
        "unknown".into()
    }
}

fn kaynak_ref(kok: &Path) -> String {
    // Git deposundaysa HEAD'in işaret ettiği commit; değilse "unknown"
    let mut d = Some(kok);
    while let Some(k) = d {
        // `.git` bir dosyaysa (worktree, alt modül) ref'i burada değildir: üst deponun ref'ini almak yanlış olur
        if k.join(".git").is_file() {
            return "unknown".into();
        }
        let git = k.join(".git");
        // Okunan dosyanın gerçek yolu .git'in içinde olmalı: aradaki bir klasör (ör. refs/heads) depo dışına bağ olabilir
        let git_dosyasi = |p: &Path| if calisma_alani::icinde(&git, p) { guvenli::oku(p) } else { None };
        let head = git.join("HEAD");
        if let Some(h) = git_dosyasi(&head) {
            let h = h.trim();
            if let Some(r) = h.strip_prefix("ref: ") {
                // Yalnızca refs/ altındaki adlar: "ref: ../../başka/dosya" ile depo dışı okuma yapılmasın
                if !r.starts_with("refs/") || r.split(['/', '\\']).any(|p| p == ".." || p.is_empty()) {
                    return "unknown".into();
                }
                if let Some(c) = git_dosyasi(&git.join(r)) {
                    return commit_kisa(c.trim());
                }
                if let Some(p) = git_dosyasi(&git.join("packed-refs")) {
                    if let Some(l) = p.lines().find(|l| l.ends_with(r)) {
                        return commit_kisa(l);
                    }
                }
                // Ref çözülemedi: HEAD metni ("ref: refs/...") commit değildir
                return "unknown".into();
            }
            return commit_kisa(h);
        }
        d = k.parent();
    }
    "unknown".into()
}

fn simdi_sn() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64
}

fn simdi() -> String {
    benlik::zaman_metni(simdi_sn())
}

fn json_oku(p: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_default()).unwrap_or_default()
}

/// `sezgi dogrula`: kartın 0.1.1 alanları. Geçerlilik süresi, kanıt özetlerinin Merkle kökü, kartın yanındaki kanıt
/// paketi (kayıt özetleri ve paketin kendi imzası) ve kart bir projenin `.sezgi/` klasöründeyse bayat işlemler.
/// Bayatlık uyarıdır (kart doğru imzalı ama kod değişmiş): sonucu değiştirmez. Dönen: sorun yoksa true
fn kart_ekleri_dogrula(kart: &serde_json::Value, kart_yolu: &Path, anahtarlar: &serde_json::Value) -> bool {
    let mut tamam = true;
    let (verilis, bitis) = (kart["x-issued_at"].as_str(), kart["x-expires_at"].as_str());
    match bitis.map(|b| (b, benlik::zaman_sn(b))) {
        None => tp!("· Geçerlilik süresi yok (0.1 kartı)", "· No validity period (0.1 card)"),
        Some((b, None)) => {
            tp!("✗ Geçerlilik bitişi okunamadı: {b}", "✗ Could not read the validity end: {b}");
            tamam = false;
        }
        Some((b, Some(s))) if s <= simdi_sn() => {
            tp!("✗ Kartın süresi dolmuş ({b}); yeniden üretilmeli: sezgi init", "✗ The card has expired ({b}); generate it again: sezgi init");
            tamam = false;
        }
        Some((b, Some(s))) => {
            let gun = (s - simdi_sn()) / 86_400;
            tp!("✓ Geçerlilik: {} → {b} ({gun} gün kaldı)", "✓ Validity: {} → {b} ({gun} {g} left)", verilis.unwrap_or("?"); g = crate::dil::cogul(gun, "day", "days"));
        }
    }
    if benlik::kok_tutarli(kart) == Some(false) {
        tp!(
            "✗ Kanıt özetlerinin Merkle kökü uyuşmuyor (x-evidence_root): kart tutarsız",
            "✗ The Merkle root of the evidence digests does not match (x-evidence_root): the card is inconsistent"
        );
        tamam = false;
    }
    // Kanıt paketi: yalnızca kartın yanındaki düz dosya adı (yol bileşeni kabul edilmez)
    if let Some(ad) = kart["x-evidence_bundle"].as_str() {
        let duz = !ad.is_empty() && !ad.contains(['/', '\\']) && ad != "." && ad != "..";
        let yol = kart_yolu.parent().map(|d| d.join(ad));
        match yol.filter(|_| duz).and_then(|y| guvenli::oku(&y)).and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok()) {
            None => tp!("· Kanıt paketi sunulmadı ({ad}); yalnızca karttaki özetler denetlendi", "· No evidence bundle ({ad}); only the digests in the card were checked"),
            Some(paket) => {
                let (gecerli, sunulmayan, hatalar) = benlik::kanit_paketi_denetle(kart, &paket);
                for h in gruplu_hatalar(&hatalar) {
                    tp!("✗ Kanıt paketi: {h}", "✗ Evidence bundle: {h}");
                }
                if !hatalar.is_empty() {
                    tamam = false;
                }
                if !sunulmayan.is_empty() {
                    tp!("! Kanıt paketinde {} kayıt sunulmadı: {}", "! {} {k} missing from the evidence bundle: {}", sunulmayan.len(), ornekli(&sunulmayan, 3); k = crate::dil::cogul(sunulmayan.len(), "record", "records"));
                }
                if paket["signature"].is_object() {
                    match imza::dogrula_baglamla(&paket, anahtarlar, imza::KANIT_BAGLAMI) {
                        Ok(k) => tp!("✓ Kanıt paketi imzası geçerli (anahtar '{k}')", "✓ Evidence bundle signature valid (key '{k}')"),
                        Err(e) => {
                            tp!("✗ Kanıt paketi imzası: {e}", "✗ Evidence bundle signature: {e}");
                            tamam = false;
                        }
                    }
                } else {
                    tp!("· Kanıt paketi imzasız", "· Evidence bundle unsigned");
                }
                if hatalar.is_empty() {
                    tp!("✓ Kanıt paketi: {gecerli} kayıt geçerli", "✓ Evidence bundle: {gecerli} {k} valid"; k = crate::dil::cogul(gecerli, "record", "records"));
                }
            }
        }
    }
    // Bayatlık: kart <proje>/.sezgi/ içindeyse dayanak özetleri projenin güncel dosyalarıyla karşılaştırılır
    let klasor = kart_yolu.parent().and_then(|d| std::fs::canonicalize(d).ok());
    if let Some(kok) = klasor.as_ref().filter(|d| d.file_name().is_some_and(|a| a == ".sezgi")).and_then(|d| d.parent()) {
        let bayat = benlik::bayat_islemler(kart, kok);
        if !bayat.is_empty() {
            tp!(
                "! {} işlem bayat: dayandığı kaynak kod kart üretildikten sonra değişti ({}); yeniden üretin: sezgi init",
                "! {} {i} stale: {k} source code changed after the card was generated ({}); generate it again: sezgi init",
                bayat.len(),
                ornekli(&bayat, 8);
                i = crate::dil::cogul(bayat.len(), "operation is", "operations are"),
                k = crate::dil::cogul(bayat.len(), "its", "their")
            );
        } else if kart.get("x-basis").is_some() {
            tp!("✓ Kart güncel: dayandığı kaynak dosyalar değişmedi", "✓ Card up to date: its source files have not changed");
        }
    }
    tamam
}

/// Uzun listenin ilk birkaç öğesi ve kalanın sayısı: "a, b, c, +51 daha"
fn ornekli(liste: &[String], en_fazla: usize) -> String {
    let mut v: Vec<String> = liste.iter().take(en_fazla).cloned().collect();
    if liste.len() > en_fazla {
        v.push(tf!("+{} daha", "+{} more", liste.len() - en_fazla));
    }
    v.join(", ")
}

/// Aynı nedenli kayıt hataları ("<kimlik>: <neden>") tek satırda: neden, sayı ve ilk birkaç kimlik. Kimliksiz hata
/// olduğu gibi kalır
fn gruplu_hatalar(hatalar: &[String]) -> Vec<String> {
    let mut gruplar: Vec<(String, Vec<String>)> = vec![];
    let mut tek: Vec<String> = vec![];
    for h in hatalar {
        match h.split_once(": ").filter(|(id, _)| !id.is_empty() && !id.contains(' ')) {
            Some((id, neden)) => match gruplar.iter_mut().find(|(n, _)| n == neden) {
                Some((_, l)) => l.push(id.to_string()),
                None => gruplar.push((neden.to_string(), vec![id.to_string()])),
            },
            None => tek.push(h.clone()),
        }
    }
    tek.extend(gruplar.into_iter().map(|(neden, l)| {
        if l.len() == 1 {
            format!("{}: {neden}", l[0])
        } else {
            tf!("{neden}: {} kayıt ({})", "{neden}: {} {k} ({})", l.len(), ornekli(&l, 3); k = crate::dil::cogul(l.len(), "record", "records"))
        }
    }));
    tek
}

fn secenek(args: &[String], ad: &str) -> Option<String> {
    args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned()
}

/// Çıktıyı okuyan taraf kapandıysa (`sezgi rapor | head`) yazma panik vermez: süreç sessizce 0 ile çıkar.
/// Rust SIGPIPE'ı yok sayar; `println!` başarısız yazmada "failed printing to stdout" ile panikler. Bu iletiyle
/// başlamayan her panik olduğu gibi raporlanır.
fn cikti_kapaninca_sessiz_cik() {
    let onceki = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |bilgi| {
        let ileti = bilgi.payload().downcast_ref::<String>().map(String::as_str).or_else(|| bilgi.payload().downcast_ref::<&str>().copied()).unwrap_or("");
        if ileti.starts_with("failed printing to stdout") {
            std::process::exit(0);
        }
        onceki(bilgi);
    }));
}

fn main() {
    // İngilizce takma adlar Türkçe asıllarına çevrilir: `sezgi key create --name k` = `sezgi anahtar olustur --ad k`
    // Yalnızca komut konumu ve bayraklar çevrilir; "report" adlı bir uygulama klasörü olduğu gibi kalır
    cikti_kapaninca_sessiz_cik();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Çıktı dili (--lang en|tr, SEZGI_LANG, sistem dili): ilk çıktıdan önce; bayrak satırdan çıkarılır
    dil::kur(&mut args);
    let komut_en = |a: &str| match a {
        "key" => Some("anahtar"),
        "verify" => Some("dogrula"),
        "report" => Some("rapor"),
        "mirror" => Some("ayna"),
        "ekle" => Some("add"),
        "doctor" => Some("doktor"),
        "measure" => Some("olc"),
        "share" => Some("paylas"),
        "memory" => Some("hafiza"),
        "diff" => Some("karsilastir"),
        "discover" => Some("kesfet"),
        "serve" => Some("sun"),
        "remove" | "uninstall" => Some("kaldir"),
        "watch" | "izle" => Some("calistir"),
        "mode" => Some("mod"),
        "target" => Some("hedef"),
        "session" => Some("oturum"),
        "approve" | "approval" => Some("onay"),
        "log" => Some("kayit"),
        "ledger" => Some("defter"),
        "human" => Some("insan"),
        "forget" => Some("unut"),
        "diagnostics" => Some("teshis"),
        "switchboard" => Some("santral"),
        "live" => Some("canli"),
        "observe" | "observation" => Some("gozlem"),
        "help" | "--help" | "-h" => Some("yardim"),
        _ => None,
    };
    // Argümansız: durum ve sonraki adım
    if args.is_empty() {
        yardim::ana_ekran();
        return;
    }
    // Kısa ad: `sezgi bagla` = `sezgi add mcp`; `sezgi add` ve `sezgi ekle` de aynı komut (eski önerilerde geçer)
    if matches!(args[0].as_str(), "bagla" | "connect") || (matches!(args[0].as_str(), "add" | "ekle") && args.get(1).map(String::as_str) != Some("mcp")) {
        args.splice(0..1, ["add".to_string(), "mcp".to_string()]);
    }
    if let Some(t) = args.first().and_then(|a| komut_en(a)) {
        args[0] = t.to_string();
    }
    for a in args.iter_mut() {
        if let Some(t) = match a.as_str() {
            "--out" => Some("--cikti"),
            "--sign" => Some("--imzala"),
            "--name" => Some("--ad"),
            "--tool" => Some("--arac"),
            "--map" => Some("--harita"),
            "--strict" => Some("--sert"),
            "--dry-run" => Some("--deneme"),
            "--frontend" => Some("--on-yuz"),
            "--project" => Some("--proje"),
            "--steer" => Some("--yonlendir"),
            "--manual" => Some("--elle"),
            "--delete-map" => Some("--harita-sil"),
            "--yes" => Some("--evet"),
            "--all" => Some("--hepsi"),
            "--person" => Some("--kisi"),
            "--system" => Some("--sistem"),
            "--user" => Some("--kullanici"),
            _ => None,
        } {
            *a = t.to_string();
        }
    }
    // Yardım, İngilizce alt komutlar ve bayrak denetimi: komut çalışmadan, hiçbir dosyaya dokunmadan
    yardim::denetle(&mut args);
    match args.first().map(String::as_str) {
        Some("--version") | Some("-v") => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Some("init") => {
            init(&args, &mut Onbellek::default(), true);
        }
        Some("calistir") => {
            izle::calistir(&args);
        }
        Some("add") if args.get(1).map(String::as_str) == Some("mcp") => {
            baglan::ekle_mcp(&args);
        }
        Some("doktor") => {
            doktor::calistir(&args);
        }
        Some("olc") => {
            olc::calistir(&args);
        }
        Some("paylas") => {
            paylas::calistir(&args);
        }
        Some("karsilastir") => {
            fark::calistir(&args);
        }
        Some("kesfet") => kesif::masaustu::komut(&args),
        Some("rea") => kesif::rea_kur::komut(&args),
        Some("mod") => canli::mod_komutu(&args),
        Some("hedef") => canli::hedef_komutu(&args),
        Some("token") => canli::token_komutu(&args),
        Some("oturum") => canli::oturum_komutu(&args),
        Some("canli") => canli::canli_komutu(&args),
        Some("onay") => canli::onay_komutu(&args),
        Some("plan") => plan::komut(&args),
        Some("kayit") => canli::kayit_komutu(&args),
        Some("insan") => insan::insan_komutu(&args),
        Some("gozlem") => gozlem::komut(&args),
        Some("defter") => canli::defter_komutu(&args),
        Some("unut") => {
            // Bütün uygulamalardaki müşteri verisi geri alınamaz biçimde silinir: açık onay
            if args.iter().any(|a| a == "--hepsi")
                && !baglan::onaylandi(
                    &args,
                    t!(
                        "Bütün uygulamalardaki müşteri verisi (bekleyen isteklerin gövdeleri, deneyim hafızası) silinsin mi?",
                        "Delete the customer data of all applications (bodies of pending requests, experience memory)?"
                    ),
                )
            {
                te!(
                    "✗ Hiçbir şey silinmedi. Terminal dışında onay için: sezgi unut --hepsi --evet",
                    "✗ Nothing was deleted. To confirm outside a terminal: sezgi forget --all --yes"
                );
                std::process::exit(2);
            }
            canli::unut_komutu(&args)
        }
        Some("teshis") => teshis::komut(&args),
        Some("santral") => santral::komut(&args),
        Some("hafiza") => hafiza::komut(&args),
        Some("sun") => sun::calistir(&args),
        Some("kaldir") => {
            baglan::kaldir(&args);
        }
        Some("mcp") => {
            let verilen = args.get(1).filter(|a| !a.starts_with("--")).cloned().or_else(|| secenek(&args, "--harita"));
            let klasor = guvenli::harita_bul(verilen.as_deref());
            // İngilizce (--lang en, SEZGI_LANG ya da sistem dili): araçlar İngilizce adlarıyla listelenir (iki ad takımı da
            // her zaman çağrılabilir)
            let ingilizce = dil::en();
            // --santral: santraldeki bütün uygulamalar tek sunucuda
            if args.iter().any(|a| a == "--santral") {
                santral::mcp(ingilizce);
            } else {
                mcp::calistir(&klasor, ingilizce);
            }
        }
        Some("anahtar") if args.get(1).map(String::as_str) == Some("olustur") => {
            let ad = secenek(&args, "--ad").unwrap_or_else(|| "varsayilan".into());
            match imza::anahtar_olustur(&ad) {
                Ok((acik, _)) => {
                    tp!("✓ ML-DSA-65 anahtarı oluşturuldu: '{ad}'", "✓ ML-DSA-65 key created: '{ad}'");
                    tp!(
                        "  Gizli anahtar: {} (proje klasörüne kopyalama, kimseyle paylaşma)",
                        "  Private key:   {} (do not copy it into the project folder, do not share it)",
                        imza::anahtar_klasoru().display()
                    );
                    tp!("  Açık anahtar:  {}", "  Public key:    {}", acik.display());
                    tp!(
                        "  Yayın:         bu açık anahtar belgesini uygulamanın /.well-known/benlik-keys.json adresinde yayınla",
                        "  Publish:       serve this public key document at the app's /.well-known/benlik-keys.json"
                    );
                }
                Err(e) => {
                    eprintln!("✗ {e}");
                    std::process::exit(1);
                }
            }
        }
        // Çalışan Ayna'ların anahtarlı adresleri (Ayna yeniden başlatılmadan)
        Some("ayna") if args.iter().any(|a| a == "--adres") => ayna::adresler(),
        Some("ayna") => {
            // Verilen klasörde harita yoksa hata (yazım hatası sessizce tanıtıma düşmez); verilmediyse ve bulunulan
            // klasörde ya da üstlerinde harita yoksa Ayna haritasız açılır: Sezgi'yi tanıtır ve ilk adımı gösterir
            let verilen = args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str);
            let klasor = if verilen.is_some() { guvenli::harita_gerekli(verilen) } else { guvenli::harita_bul(None) };
            let port = secenek(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(4777);
            ayna::calistir(&klasor, port);
        }
        Some("rapor") => {
            let klasor = guvenli::harita_gerekli(args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str));
            match rapor::uret(&klasor) {
                Ok(r) if args.iter().any(|a| a == "--json") => println!("{}", serde_json::to_string_pretty(&r).unwrap()),
                Ok(r) => rapor::yazdir(&r),
                Err(e) => {
                    eprintln!("✗ {e}");
                    std::process::exit(1);
                }
            }
        }
        // anahtar sabitle <benlik.json> <benlik-keys.json>: imza doğrulanırsa anahtarı kartın uygulaması için sabitler
        Some("anahtar") if args.get(1).map(String::as_str) == Some("sabitle") => {
            let (Some(kart_yolu), Some(anahtar_yolu)) = (args.get(2), args.get(3)) else { yardim::eksik("anahtar") };
            match imza::anahtar_sabitle(&json_oku(kart_yolu), &json_oku(anahtar_yolu)) {
                Ok((uygulama, k)) => {
                    tp!("✓ '{uygulama}' için anahtar '{k}' sabitlendi", "✓ Key '{k}' pinned for '{uygulama}'");
                    tp!(
                        "  Bundan sonra bu uygulamanın kartı yalnızca bu anahtarla imzalıysa güvenilir sayılır (sezgi dogrula, sezgi ayna).",
                        "  From now on this app's card is trusted only if it is signed with this key (sezgi verify, sezgi mirror)."
                    );
                    tp!(
                        "  Anahtarı uygulama sahibinin yayınladığı /.well-known/benlik-keys.json ile karşılaştırdıysanız sabitleyin.",
                        "  Pin it only after comparing it with the /.well-known/benlik-keys.json the app's owner publishes."
                    );
                }
                Err(e) => {
                    te!("✗ Sabitlenmedi: {e}", "✗ Not pinned: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("dogrula") => {
            let (Some(kart_yolu), Some(anahtar_yolu)) = (args.get(1), args.get(2)) else { yardim::eksik("dogrula") };
            let (kart, anahtarlar) = (json_oku(kart_yolu), json_oku(anahtar_yolu));
            // Bütün sorunlar söylenir, sonra karar verilir: imza, geçerlilik süresi, kanıt kökü, kanıt paketi
            let mut basarisiz = false;
            match imza::dogrula(&kart, &anahtarlar) {
                Ok(k) => match imza::guven_durumu(&kart, &anahtarlar) {
                    imza::Guven::Sabit => tp!("✓ İmza geçerli ve güvenilir (ML-DSA-65, sabitlenmiş anahtar '{k}')", "✓ Signature valid and trusted (ML-DSA-65, pinned key '{k}')"),
                    imza::Guven::SabitUyusmaz => {
                        tp!(
                            "✗ İmza kendi içinde tutarlı ama anahtar '{k}' bu uygulama için sabitlenmiş anahtarlardan biri değil",
                            "✗ The signature is consistent, but key '{k}' is not one of the keys pinned for this app"
                        );
                        basarisiz = true;
                    }
                    imza::Guven::Degisti => {
                        tp!(
                            "! İmza kendi içinde tutarlı (anahtar '{k}') ama bu uygulama için daha önce başka bir anahtar görüldü: anahtar değişti.",
                            "! The signature is consistent (key '{k}'), but a different key was seen for this app before: the key changed."
                        );
                        tp!(
                            "  Yeni anahtarı uygulama sahibinden doğrulayın; doğruysa: sezgi anahtar sabitle <benlik.json> <benlik-keys.json>",
                            "  Confirm the new key with the app's owner; if it is right: sezgi key pin <benlik.json> <benlik-keys.json>"
                        );
                    }
                    imza::Guven::Bilinmiyor => {
                        if let Err(e) = imza::gorulen_kaydet(&kart, &anahtarlar) {
                            te!("! Görülen anahtar kaydedilemedi: {e}", "! Could not record the seen key: {e}");
                        }
                        tp!(
                            "✓ İmza geçerli (ML-DSA-65, anahtar '{k}'); anahtar sabitlenmediği için kimlik doğrulanmadı",
                            "✓ Signature valid (ML-DSA-65, key '{k}'); the key is not pinned, so the identity is not verified"
                        );
                    }
                },
                Err(e) => {
                    println!("✗ {e}");
                    basarisiz = true;
                }
            }
            if !kart_ekleri_dogrula(&kart, Path::new(kart_yolu), &anahtarlar) {
                basarisiz = true;
            }
            if basarisiz {
                std::process::exit(1);
            }
        }
        Some("yardim") => yardim::yardim(&args),
        Some(x) => yardim::bilinmeyen(x),
        None => yardim::ana_ekran(),
    }
}

pub struct InitOzet {
    pub endpoint: usize,
    pub dosya: usize,
    pub ayristirilan: usize,
    pub sure: std::time::Duration,
}

/// Çıktı klasörünü yazmaya hazırlar. Klasör ya da yazılacak dosyalar sembolik bağ olamaz: kötü niyetli bir depo
/// `.sezgi` adlı bir bağla Sezgi'nin başka bir yere yazmasını sağlayamasın. Varsayılan klasör uygulamanın içindeyse
/// (`.sezgi`) kendini git'ten gizler: haritalar yanlışlıkla depoya girmesin
fn cikti_hazirla(kok: &Path, cikti: &Path) {
    let bag_mi = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if bag_mi(cikti)
        || ["kapilar.json", "benlik.json", "benlik-keys.json", "openapi.json", "web-baglari.json", benlik::KANIT_PAKETI, kanit::DOSYA, graf::DOSYA, gozlem::DOSYA]
            .iter()
            .any(|d| bag_mi(&cikti.join(d)))
    {
        te!("✗ Çıktı yolu sembolik bağ içeriyor, yazılmadı: {}", "✗ The output path contains a symbolic link; nothing written: {}", cikti.display());
        std::process::exit(1);
    }
    if let Err(e) = std::fs::create_dir_all(cikti) {
        te!("✗ Çıktı klasörü oluşturulamadı: {e}", "✗ Could not create the output folder: {e}");
        std::process::exit(1);
    }
    if cikti.starts_with(kok) && !cikti.join(".gitignore").exists() && !bag_mi(&cikti.join(".gitignore")) {
        if let Err(e) = std::fs::write(cikti.join(".gitignore"), "# sezgi çıktısı: depoya eklenmez\n*\n") {
            te!(
                "! {} yazılamadı ({e}); çıktı klasörünü depoya eklememeye dikkat edin",
                "! Could not write {} ({e}); take care not to commit the output folder",
                cikti.join(".gitignore").display()
            );
        }
    }
}

/// Bir dilin keşif, ayrıştırma ve adaptör sonucu; `init` dilden bağımsız kısmı (harita, kart, OpenAPI, imza) bundan yazar
pub struct Toplam {
    pub kapilar: Vec<adaptor::Kapi>,
    pub bilgi: serde_json::Value,
    pub adaptor: String,
    pub dosya_sayisi: usize,
    pub satir: usize,
    pub kismi: Vec<String>,
    pub atlanan: usize,
    pub buyuk: Vec<String>,
    pub ek_paketler: Vec<String>,
    pub veri_dosyalari: Vec<String>,
    pub ayristirilan: usize,
    /// Adaptörün okuduğu dosyalar ve içerik özetleri (mutlak yol); haritanın `okunan_dosyalar` alanı
    pub okunan: Vec<(PathBuf, String)>,
    pub t_kesif: std::time::Duration,
    pub t_ayristirma: std::time::Duration,
    pub t_toplam: std::time::Duration,
}

/// TypeScript + NestJS: keşif, paralel ayrıştırma (önbellekli) ve adaptör
pub(crate) fn ts_topla(kok: &Path, kurallar: &adaptor::Kurallar, pkg: &str, onbellek: &mut HashMap<PathBuf, kesif::Kayit<model::Dosya>>, t0: Instant) -> Toplam {
    let kok = kok.to_path_buf();
    // 1) Keşif: .gitignore'a uyarak TypeScript dosyalarını bul
    let mut yollar: Vec<PathBuf> = Vec::new();
    let mut atlanan = 0usize;
    let mut buyuk: Vec<PathBuf> = Vec::new();
    // ORM şema dosyaları (ör. schema.prisma): veritabanı tablosu bağları için
    let veri_uzantisi = kurallar.veri.as_ref().map(|v| v.sema_uzantisi.clone()).unwrap_or_default();
    let mut veri_dosyalari: Vec<PathBuf> = Vec::new();
    let mut tara = |klasor: &Path, yollar: &mut Vec<PathBuf>| {
        for g in ignore::WalkBuilder::new(klasor).hidden(true).git_ignore(true).build().flatten() {
            let p = g.path();
            // Sembolik bağlar izlenmez: depo dışındaki bir dosyayı (ör. ~/.ssh) gösterebilir
            if g.path_is_symlink() {
                atlanan += 1;
                continue;
            }
            if !p.is_file() {
                continue;
            }
            // Dev dosyalar (üretilmiş kod, gömülü veri) belleği doldurmasın: atlanır ve raporlanır
            if g.metadata().map(|m| m.len() > EN_BUYUK_DOSYA).unwrap_or(false) {
                buyuk.push(p.to_path_buf());
                continue;
            }
            let s = p.to_string_lossy();
            if s.contains("node_modules") || s.contains("\\dist\\") || s.contains("/dist/") {
                atlanan += 1;
                continue;
            }
            match p.extension().and_then(|e| e.to_str()) {
                Some("ts") | Some("tsx") if !s.ends_with(".d.ts") && !s.ends_with(".spec.ts") => yollar.push(p.to_path_buf()),
                Some(e) if !veri_uzantisi.is_empty() && e == veri_uzantisi => veri_dosyalari.push(p.to_path_buf()),
                _ => atlanan += 1,
            }
        }
    };
    tara(&kok, &mut yollar);
    let uygulama_dosya_sayisi = yollar.len();
    // Monorepo: uygulamanın bağımlı olduğu kardeş paketler (girdi tanımları, şemalar, sabitler için)
    let ek_paketler = calisma_alani::bagimli_paketler(&kok);
    for (_, klasor) in &ek_paketler {
        tara(klasor, &mut yollar);
    }
    let t_kesif = t0.elapsed();

    // 2) Paralel ayrıştırma: her dosya bir kez, bütün çekirdeklerde aynı anda; değişmeyen dosya önbellekten
    let ek_paket: std::collections::HashSet<PathBuf> = yollar[uygulama_dosya_sayisi..].iter().cloned().collect();
    let (sonuclar, mut okunan, ayristirilan) = kesif::ayristir_hepsi(&yollar, onbellek, model::ayristir_metin);
    let mut dosyalar: Vec<model::Dosya> = sonuclar
        .into_iter()
        .map(|(p, mut d)| {
            d.ek_paket = ek_paket.contains(&p);
            d
        })
        .collect();
    dosyalar.sort_by_key(|d| d.ek_paket);
    let t_ayristirma = t0.elapsed();

    // 3) Adaptör: kapılar, girdiler, kurallar, yetki
    // package.json'daki şema yolu (ör. "prisma": { "schema": "../../packages/prisma/schema.prisma" }): dosya ya da klasör
    if let Some(v) = &kurallar.veri {
        let paket: serde_json::Value = serde_json::from_str(pkg).unwrap_or_default();
        let deger = v.paket_ayari.iter().try_fold(&paket, |d, a| d.get(a));
        if let Some(yol) = deger.and_then(|d| d.as_str()) {
            let hedef = kok.join(yol);
            // Şema yolu deponun (çalışma alanının) dışına çıkamaz; dosya ise uzantısı .prisma olmalı
            let sinir = git_koku(&kok).unwrap_or_else(|| kok.clone());
            let hedef = if calisma_alani::icinde(&sinir, &hedef) { hedef } else { PathBuf::new() };
            let hedefler: Vec<PathBuf> = if hedef.is_dir() {
                std::fs::read_dir(&hedef)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(v.sema_uzantisi.as_str()))
                    // Klasördeki bağlar izlenmez, dosyalar da sınırın içinde olmalı
                    .filter(|p| std::fs::symlink_metadata(p).map(|m| m.file_type().is_file()).unwrap_or(false) && calisma_alani::icinde(&sinir, p))
                    .collect()
            } else {
                vec![hedef]
            };
            for h in hedefler {
                let ayni = |a: &Path| std::fs::canonicalize(a).ok() == std::fs::canonicalize(&h).ok();
                let uzanti_dogru = h.extension().and_then(|e| e.to_str()) == Some(v.sema_uzantisi.as_str());
                if uzanti_dogru
                    && std::fs::symlink_metadata(&h).map(|m| m.file_type().is_file()).unwrap_or(false)
                    && h.metadata().map(|m| m.len() <= EN_BUYUK_DOSYA).unwrap_or(false)
                    && !veri_dosyalari.iter().any(|p| ayni(p))
                {
                    veri_dosyalari.push(h);
                }
            }
        }
    }
    let mut kaskadlar = vec![];
    let veri_metinleri: Vec<String> = veri_dosyalari
        .iter()
        .filter_map(|p| {
            let m = guvenli::oku(p)?;
            okunan.push((p.clone(), kesif::icerik_ozeti(m.as_bytes())));
            kaskadlar.extend(dna::prisma_kaskadlari(&guvenli::goreli(&kok, p), &m));
            Some(m)
        })
        .collect();
    let motor = adaptor::Motor::yeni(kurallar, &kok, &dosyalar).veri_semalari(&veri_metinleri);
    let (kapilar, mut uygulama_bilgi) = motor.kapilar();
    if let Some(l) = uygulama_bilgi["kaskadlar"].as_array_mut() {
        l.extend(kaskadlar.into_iter().map(|(e, c, k)| json!([e, c, k])));
    }
    let t_toplam = t0.elapsed();
    let satir: usize = dosyalar.iter().map(|d| d.satir_sayisi).sum();
    let kismi = dosyalar.iter().filter(|d| !d.hatali_satirlar.is_empty()).map(|d| guvenli::goreli(&kok, &d.yol)).collect();
    Toplam {
        kapilar,
        bilgi: uygulama_bilgi,
        adaptor: format!("{}@{}", kurallar.adaptor.ad, kurallar.adaptor.surum),
        dosya_sayisi: dosyalar.len(),
        satir,
        kismi,
        atlanan,
        buyuk: buyuk.iter().map(|p| guvenli::goreli(&kok, p)).collect(),
        ek_paketler: ek_paketler.iter().map(|(a, _)| a.clone()).collect(),
        veri_dosyalari: veri_dosyalari.iter().map(|p| guvenli::goreli(&kok, p)).collect(),
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

/// Sezgi'nin kaynak kodundan okuyabildiği çatılar (init ve doktor aynı listeyi söyler)
pub(crate) const DESTEKLENEN_CATILAR: &str = "TypeScript + NestJS, Python + FastAPI, Django (Django REST Framework), Flask, JavaScript / TypeScript + Express, Fastify, Next.js, Java / Kotlin + Spring Boot, Kotlin + Ktor, Ruby on Rails, C# + ASP.NET Core, Go + net/http, Gin, Echo, Chi, Fiber, gorilla/mux, PHP + Laravel";

/// Desteklenen uygulama bulunamadı: neye bakıldığı ve alt klasörlerdeki adaylar (en çok 3 kat, bağımlılık ve derleme
/// klasörleri hariç)
fn desteklenmeyen_klasor(kok: &Path) -> ! {
    te!("✗ Bu klasörde Sezgi'nin okuyabildiği bir uygulama bulunamadı.", "✗ No application Sezgi can read was found in this folder.");
    te!(
        "  Bakılan: package.json'da @nestjs/core, express, fastify ya da next; pyproject.toml / requirements.txt'te fastapi, django ya da flask; manage.py; pom.xml / build.gradle'da spring-boot ya da io.ktor; Gemfile'da rails (config/routes.rb ile); .csproj'ta Microsoft.NET.Sdk.Web; go.mod; composer.json'da laravel/framework.",
        "  Looked for: @nestjs/core, express, fastify or next in package.json; fastapi, django or flask in pyproject.toml / requirements.txt; manage.py; spring-boot or io.ktor in pom.xml / build.gradle; rails in Gemfile (with config/routes.rb); Microsoft.NET.Sdk.Web in .csproj; go.mod; laravel/framework in composer.json."
    );
    let atla = ["node_modules", ".git", "dist", "build", "target", ".venv", "venv", ".next", "coverage", "vendor"];
    let mut adaylar = vec![];
    let mut kuyruk = vec![(kok.to_path_buf(), 0)];
    while let Some((k, derinlik)) = kuyruk.pop() {
        let Ok(oge) = std::fs::read_dir(&k) else { continue };
        let mut alt: Vec<PathBuf> =
            oge.flatten().map(|g| g.path()).filter(|p| p.is_dir() && !p.file_name().is_some_and(|a| atla.contains(&a.to_string_lossy().as_ref()))).collect();
        alt.sort();
        for a in alt {
            if let Some(c) = cati(&a) {
                adaylar.push((a.strip_prefix(kok).unwrap_or(&a).to_string_lossy().replace('\\', "/"), c));
            } else if derinlik < 2 {
                kuyruk.push((a, derinlik + 1));
            }
        }
    }
    adaylar.sort();
    if adaylar.is_empty() {
        te!("  Desteklenenler: {DESTEKLENEN_CATILAR}.", "  Supported: {DESTEKLENEN_CATILAR}.");
    } else {
        te!("  Alt klasörlerde bulunanlar:", "  Found in subfolders:");
        for (yol, c) in adaylar.iter().take(5) {
            eprintln!("    sezgi init {yol:<24} ({c})");
        }
        if adaylar.len() > 5 {
            te!("    … {} aday daha", "    … {} more {a}", adaylar.len() - 5; a = crate::dil::cogul(adaylar.len() - 5, "candidate", "candidates"));
        }
    }
    std::process::exit(1);
}

/// Haritadan kanıt deposu (`kanit.json`) ve graf (`graf.json`): adaptör çıktısının ve web adaptörünün ön yüz
/// bağlarının (`web`; yoksa Null) kanıta çevrilmiş hâli. Kart kanıtlarını bu depodan alır
fn kanit_yaz(harita: &serde_json::Value, web: &serde_json::Value, ek: &serde_json::Value, on_yuz: Option<&Path>, kok: &Path, cikti: &Path) -> kanit::Depo {
    // Ön yüz kanıtlarının dosyaları ön yüz klasörüne görelidir: dayanak yolları uygulama köküne göre yazılır
    let onek = on_yuz.and_then(|o| kanit::goreli_yol(kok, o)).filter(|o| o != ".").unwrap_or_default();
    // Çalışma zamanı gözlemleri (`sezgi gozlem al`, canlı mod): her kurulumda kanıt deposuna yeniden girer
    let gozlemler = gozlem::Gozlemler::oku(cikti);
    let mut depo = kanit::uret(harita, web, &onek, ek, Some(kok), &simdi(), gozlemler.as_ref());
    depo.kok = kanit::kok_goreli(&std::fs::canonicalize(kok).unwrap_or_else(|_| kok.to_path_buf()), &std::fs::canonicalize(cikti).unwrap_or_else(|_| cikti.to_path_buf()));
    if let Err(e) = kanit::yaz(cikti, &mut depo) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
    depo
}

fn init(args: &[String], onbellek: &mut Onbellek, ayrintili: bool) -> Option<InitOzet> {
    // Uygulama klasörü verilmezse bulunulan klasör
    let uygulama = args.get(1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| ".".into());
    let deneme = args.iter().any(|a| a == "--deneme" || a == "--dry-run");
    let kok = std::fs::canonicalize(&uygulama).unwrap_or_else(|_| PathBuf::from(&uygulama));
    let cikti = args.iter().position(|a| a == "--cikti").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(|| kok.join(".sezgi"));
    // Ön yüz (P2.1): --on-yuz ile verilir; verilmezse monorepoda kendiliğinden aranır (uygulama, altı ya da yanı)
    let on_yuz_acik = args.iter().any(|a| a == "--on-yuz");
    let on_yuz = match args.iter().position(|a| a == "--on-yuz").and_then(|i| args.get(i + 1)) {
        Some(y) => match std::fs::canonicalize(y).ok().filter(|p| p.is_dir()) {
            Some(p) => Some(p),
            None => {
                te!("✗ --on-yuz: klasör bulunamadı: {y}", "✗ --on-yuz: folder not found: {y}");
                std::process::exit(1);
            }
        },
        None => web::on_yuz_bul(&kok, git_koku(&kok).as_deref()),
    };
    // Kartın geçerlilik süresi (gün): süresi dolan kartla canlı mod yazmaz (okumada uyarır), sezgi dogrula hata verir;
    // kart yeniden üretilmeli
    let gecerlilik = match secenek(args, "--gecerlilik").map(|g| g.parse::<u64>()) {
        None => benlik::VARSAYILAN_GECERLILIK_GUN,
        Some(Ok(g)) if (1..=365).contains(&g) => g,
        Some(_) => {
            te!("✗ --gecerlilik 1 ile 365 arasında bir gün sayısı ister", "✗ --gecerlilik needs a number of days between 1 and 365");
            std::process::exit(2);
        }
    };

    // Verilen yol yoksa "uygulama bulunamadı" yerine bunu söyle
    if !kok.is_dir() {
        let neden = if kok.exists() { t!("klasör değil", "not a folder") } else { t!("klasör yok", "no such folder") };
        eprintln!("✗ {neden}: {uygulama}");
        std::process::exit(1);
    }
    let kurallar: adaptor::Kurallar = toml::from_str(NESTJS).expect("adaptör dosyası okunamadı");
    let pkg = guvenli::oku(&kok.join("package.json")).unwrap_or_default();
    let secim = cati_sec(&kok, &pkg, &kurallar).unwrap_or_else(|| desteklenmeyen_klasor(&kok));

    let t0 = Instant::now();
    // Ekibin paylaştığı harita (.sezgi/paylasim.bin): imzası sabitlenmiş anahtarla doğrulanır ve kod birebir aynıysa
    // ayrıştırma yapılmadan kullanılır; değilse nedeni söylenir ve normal kurulum yapılır
    let paylasim_dene = !deneme && onbellek.is_empty() && !args.iter().any(|a| a == "--imzala" || a == "--paylasimsiz") && kok.join(".sezgi").join(paylas::DOSYA).exists();
    if paylasim_dene {
        match paylas::dene(&kok) {
            Ok(p) => {
                cikti_hazirla(&kok, &cikti);
                let yaz = |ad: &str, v: &serde_json::Value| {
                    if let Err(e) = guvenli::atomik_yaz(&cikti.join(ad), serde_json::to_string_pretty(v).unwrap().as_bytes()) {
                        eprintln!("✗ {e}");
                        std::process::exit(1);
                    }
                };
                // Paylaşım dosyası ön yüz bağlarını taşımaz: önceki kurulumun bağları (varsa) kullanılır
                let web: serde_json::Value = guvenli::oku_harita(&cikti.join(kanit::WEB_DOSYA)).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
                kanit_yaz(&p["kapilar"], &web, &serde_json::Value::Null, on_yuz.as_deref(), &kok, &cikti);
                yaz("kapilar.json", &p["kapilar"]);
                yaz("benlik.json", &p["benlik"]);
                yaz("benlik-keys.json", &p["anahtarlar"]);
                // Paylaşım dosyası kanıt paketini taşımaz: önceki kurulumdan kalan paket bu kartla eşleşmez
                let _ = std::fs::remove_file(cikti.join(benlik::KANIT_PAKETI));
                let n = p["kapilar"]["endpointler"].as_array().map(|a| a.len()).unwrap_or(0);
                let dosya = p["dosyalar"].as_object().map(|o| o.len()).unwrap_or(0);
                if ayrintili {
                    println!("◆ Sezgi {} · {}", env!("CARGO_PKG_VERSION"), p["app"]["name"].as_str().unwrap_or(""));
                    tp!(
                        "  Harita       paylaşım dosyasından kuruldu (kod yeniden okunmadı): {n} işlem, {dosya} dosya birebir aynı",
                        "  Map          built from the share file (code not read again): {n} {i}, {dosya} {f} byte-identical";
                        i = crate::dil::cogul(n, "operation", "operations"),
                        f = crate::dil::cogul(dosya, "file", "files")
                    );
                    tp!("  İmza         sabitlenmiş anahtar '{}' ile doğrulandı", "  Signature    verified with pinned key '{}'", p["signature"]["key_id"].as_str().unwrap_or("?"));
                    tp!("  Süre         {} ms", "  Time         {} ms", t0.elapsed().as_millis());
                    tp!("  Çıktı        {}", "  Output       {}", guvenli::gorunen(&cikti));
                }
                return Some(InitOzet { endpoint: n, dosya, ayristirilan: 0, sure: t0.elapsed() });
            }
            Err(neden) => te!("! Paylaşım dosyası kullanılmadı: {neden}. Normal kurulum yapılıyor.", "! Share file not used: {neden}. Mapping from code."),
        }
    }
    let Toplam {
        mut kapilar,
        bilgi: mut uygulama_bilgi,
        adaptor: adaptor_kimligi,
        dosya_sayisi,
        satir,
        kismi,
        atlanan: _,
        buyuk,
        ek_paketler,
        mut veri_dosyalari,
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    } = kesif::topla(secim, &kok, &kurallar, &pkg, onbellek, t0);

    // --deneme: hiçbir şey yazmadan neyin, nereye ve neden yazılacağını göster
    if deneme {
        tp!("◆ Sezgi {} · deneme (hiçbir şey yazılmadı)", "◆ Sezgi {} · dry run (nothing written)", env!("CARGO_PKG_VERSION"));
        tp!("  Okunacak     {} dosya · {} işlem bulundu", "  To read      {} {f} · {} {i} found", dosya_sayisi, kapilar.len(); f = crate::dil::cogul(dosya_sayisi, "file", "files"), i = crate::dil::cogul(kapilar.len(), "operation", "operations"));
        tp!("  Yazılacak    {}", "  To write     {}", guvenli::gorunen(&cikti));
        tp!(
            "               kapilar.json  harita: işlemler, girdiler, kurallar, yetki, tablo bağları",
            "               kapilar.json  map: operations, inputs, rules, auth, table links"
        );
        tp!("               benlik.json   Benlik Kartı (uygulamanın imzalanabilir kimliği)", "               benlik.json   Benlik Card (the app's signable identity)");
        tp!("               {}  kartın kanıt kayıtları (özetleri kartta)", "               {}  the card's evidence records (their digests are in the card)", benlik::KANIT_PAKETI);
        if cikti.starts_with(&kok) {
            tp!("               .gitignore    çıktı klasörü depoya girmesin diye", "               .gitignore    keeps the output folder out of the repository");
        }
        tp!(
            "  Neden        Harita bu makineye özeldir ve uygulamanın kodundan türetilir; yeniden üretilebilir.",
            "  Why          The map belongs to this machine and is derived from the app's code; it can be rebuilt."
        );
        tp!(
            "  Dokunulmaz   Uygulamanın hiçbir dosyası değişmez; yapay zekâ araçlarının ayarlarına yazılmaz.",
            "  Untouched    No file of the app changes; AI tool settings are not written."
        );
        return None;
    }

    // 4) Çıktılar
    cikti_hazirla(&kok, &cikti);
    // Çağrı grafiği ve derin servis analizi: işlemden servis ve depo katmanına inilir; tablo etkileri, erişim denetimleri
    // ve sahiplik karşılaştırmaları davranış özetine ve veri bağına eklenir. Adaptörün okuduğu kaynaklar kullanılır
    let t_cagri = Instant::now();
    let cagri_erisimi: dis::CagriErisimi;
    let dis_izlemesi: std::collections::HashMap<String, dis_zincir::IslemDis>;
    {
        let yollar: Vec<PathBuf> = okunan.iter().map(|(p, _)| p.clone()).collect();
        let ek_modeller = uygulama_bilgi["veri_modelleri"].as_array().cloned().unwrap_or_default();
        let grafik = cagri::topla(&kok, &yollar, &mut onbellek.cagri, &ek_modeller, &veri_dosyalari);
        cagri::uygula(&grafik, &mut kapilar, &mut uygulama_bilgi);
        // Dış etkiler çağrı grafiğinin ulaştığı fonksiyonlarda da aranır (dis::isle): işlem -> (dosya, ilk, son satır, derinlik)
        // Dış etki ayrıca çağrı grafiği üzerinden izlenir (polimorfik adaylar ve kütüphane çağrıları, dis_zincir)
        dis_izlemesi = dis_zincir::hesapla(&grafik, &kapilar);
        cagri_erisimi = kapilar
            .iter()
            .filter_map(|k| {
                let h = grafik.isleyici_bul(&k.dosya, &k.fonksiyon, k.satir)?;
                let a = grafik.analiz_yontemle(h, &k.yontem);
                let l = a.ulasilan.iter().map(|u| {
                    let f = &grafik.fonksiyonlar[u.fonk];
                    (f.dosya.clone(), f.satir, f.son, u.derinlik, f.gorunen())
                });
                Some((format!("{} {}:{}", k.id, k.dosya, k.satir), l.collect()))
            })
            .collect();
        // Şema dosyası bulunamadıysa tablo tanımları ORM model dosyalarıdır (ör. Kysely tablo sınıfları)
        if veri_dosyalari.is_empty() {
            let mut d: Vec<String> = grafik.modeller.iter().filter_map(|m| m["dosya"].as_str().map(String::from)).collect();
            d.sort();
            d.dedup();
            veri_dosyalari = d;
        }
    }
    let t_cagri = t_cagri.elapsed();
    // Ön yüz bağları: ekran → UI eylemi → istemci çağrısı → bu haritanın uçları (web-baglari.json). Haritadan önce
    // kurulur: ön yüzün okunan dosyaları da haritanın okunan dosyalarına girer (bayatlık, izleme, paylaşım)
    let t_web = Instant::now();
    let mut okunan = okunan;
    let (mut ayristirilan, mut dosya_toplam) = (ayristirilan, dosya_sayisi);
    let web_baglari = on_yuz.clone().and_then(|k| {
        let ucler: Vec<web::Uc> =
            kapilar.iter().map(|x| web::Uc { yontem: x.yontem.clone(), yol: x.yol.clone(), fonksiyon: x.fonksiyon.clone(), dosya: x.dosya.clone(), satir: x.satir }).collect();
        let b = web::baglar(&k, &ucler, uygulama_bilgi["genel_onek"].as_str().unwrap_or(""), &mut onbellek.web);
        // Kendiliğinden bulunan ön yüz hiçbir API çağırmıyorsa bağ yoktur: dosya yazılmaz, özet satırı basılmaz, ön yüz
        // izlenmez. --on-yuz ile açıkça verilen ön yüzün sonucu (0 da olsa) her zaman söylenir.
        if !on_yuz_acik && b.deger["istatistik"]["istemci_cagrisi"].as_u64() == Some(0) {
            return None;
        }
        okunan.extend(b.okunan);
        okunan.sort();
        okunan.dedup_by(|a, b| a.0 == b.0);
        ayristirilan += b.ayristirilan;
        dosya_toplam += b.dosya;
        let mut w = b.deger;
        w["on_yuz"] = json!(guvenli::goreli(git_koku(&kok).as_deref().unwrap_or(&kok), &k));
        w["istatistik"]["sure_ms"] = json!(t_web.elapsed().as_millis());
        Some(w)
    });
    let on_yuz = on_yuz.filter(|_| web_baglari.is_some());
    // İç içe şemalar bütçeyle sınırlanır: kesilen ayrıntı önizlemede "kontrol edilemedi" sayılır
    let ic_kesilen = adaptor::semalari_kirp(&mut kapilar);
    let govdeli = kapilar.iter().filter(|k| !k.govde.is_empty()).count();
    let bulunamayan: Vec<&str> =
        kapilar.iter().flat_map(|k| k.govde.iter().chain(k.sorgu.iter())).filter_map(|g| g.sema.as_ref().filter(|s| s.bulunamadi).map(|s| s.dto.as_str())).collect();
    let veri_bagli = kapilar.iter().filter(|k| k.veri.as_ref().map(|v| !v.okur.is_empty() || !v.yazar.is_empty()).unwrap_or(false)).count();
    // Uygulamanın adı ve sürümü package.json'dan; yoksa klasör adı ve "unknown"
    let paket: serde_json::Value = match secim {
        s if s.python_mi() => python::paket(&kok),
        Secim::Spring => spring::paket(&kok),
        Secim::Ktor => ktor::paket(&kok),
        Secim::Rails => rails::paket(&kok),
        Secim::AspNet => aspnet::paket(&kok),
        Secim::Go => golang::paket(&kok),
        // Laravel: composer.json
        Secim::Laravel => laravel::paket(&kok),
        _ => serde_json::from_str(&pkg).unwrap_or_default(),
    };
    let ad = paket["name"].as_str().map(guvenli::temiz).filter(|a| !a.is_empty()).unwrap_or_else(|| kok.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let uygulama_surumu = paket["version"].as_str().map(guvenli::temiz).unwrap_or_else(|| "unknown".into());
    let ref_ = kaynak_ref(&kok);
    // Göç dosyalarındaki kaskadlar (ham SQL, Kysely / TypeORM göçleri, Alembic): ORM şemasındakilere eklenir; okunan
    // göç dosyaları kanıtın dayanağıdır
    {
        let goc = dna::goc_dosyalari(&kok);
        let metinler: Vec<(String, String)> = goc.iter().map(|(_, g, m)| (g.clone(), m.clone())).collect();
        let yeni = dna::goc_kaskadlari(&metinler);
        if !yeni.is_empty() {
            let mut liste = uygulama_bilgi["kaskadlar"].as_array().cloned().unwrap_or_default();
            for (e, c, k) in yeni {
                let var = liste.iter().any(|x| x[0].as_str().is_some_and(|a| a.eq_ignore_ascii_case(&e)) && x[1].as_str().is_some_and(|b| b.eq_ignore_ascii_case(&c)));
                if !var {
                    if let Some(p) = goc.iter().find(|(_, g, _)| k.starts_with(&format!("{g}:"))) {
                        if !okunan.iter().any(|(o, _)| *o == p.0) {
                            okunan.push((p.0.clone(), kesif::icerik_ozeti(p.2.as_bytes())));
                        }
                    }
                    liste.push(json!([e, c, k]));
                }
            }
            uygulama_bilgi["kaskadlar"] = json!(liste);
        }
    }
    // Davranış özeti: adaptörün zincirden okuduğu red koşulları ve iki yollu ifadeler + şemadaki kaskadlar ve geri alma
    let mut dna_haritasi = dna::tamamla(&kapilar, &uygulama_bilgi);
    let olaylar = dna::olaylari_tamamla(&uygulama_bilgi);
    // Dış etki (giden HTTP, kuyruk, e-posta, ödeme / bulut SDK'ları), geri alınabilirlik ve servis içi ön koşullar:
    // dilden bağımsız tarama, ön yüz klasörü hariç; bulgular davranış özetine işlenir, onay gerekçeleri kapılara yazılır
    let dis_tarama = dis::tara(&kok, on_yuz.as_deref());
    let (mut dis_ozeti, mut etki_onaylari) = dis::isle(&dis_tarama, &kapilar, &mut dna_haritasi, &olaylar, &cagri_erisimi, &mut okunan);
    // Çağrı grafiğiyle izlenen dış etki: sınıflandırılan kütüphane çağrıları ve polimorfik adaylarda satır desenleriyle
    // bulunanlar eklenir; zincirde sınıflandırılamayan çağrı varsa yazma işlemi onaysız gidemez (dış etki bilinmiyor)
    let dis_bilinmeyen = dis_zincir::isle(&dis_izlemesi, &dis_tarama, &kapilar, &mut dna_haritasi, &mut etki_onaylari, &mut dis_ozeti);
    drop(dis_izlemesi);
    drop(dis_tarama);
    // Haritalanmayan API türleri ve kod büyüklüğüne / rota benzeri ifadelere göre çok az işlem: init, rapor ve kimsin söyler
    let rota_benzeri = kapsam::rota_benzeri(&kok);
    let kapsam_bilgisi = json!({ "disi": kapsam::kapsam_disi(&kok), "rota_benzeri": rota_benzeri, "dusuk": kapsam::dusuk(satir, kapilar.len(), rota_benzeri) });
    let mut ozet = json!({
        "uretici": format!("sezgi {}", env!("CARGO_PKG_VERSION")),
        "adaptor": adaptor_kimligi,
        "uygulama": ad,
        "kaynak_ref": ref_,
        "genel_onek": uygulama_bilgi["genel_onek"],
        "genel_guardlar": uygulama_bilgi["genel_guardlar"],
        "ana_uygulama": uygulama_bilgi["ana_uygulama"],
        "surum_secimi": uygulama_bilgi["surum_secimi"],
        // Yalnızca bazı ortamlarda kaydedilen rotalar (ör. `if settings.DEBUG: include_router(...)`)
        "kosullu_kapilar": uygulama_bilgi.get("kosullu_kapilar").cloned().unwrap_or_else(|| json!([])),
        // Önizlemenin sınayamadığı kurallar (nesne düzeyi doğrulama, dosya yüklemesi), kapının "dosya:satır" anahtarıyla
        "denetlenemeyen": uygulama_bilgi.get("denetlenemeyen").cloned().unwrap_or_else(|| json!({})),
        // Uygulamanın kendi işlem kimlikleri (OpenAPI operationId), kapının "dosya:satır" anahtarıyla
        "islem_kimlikleri": uygulama_bilgi.get("islem_kimlikleri").cloned().unwrap_or_else(|| json!({})),
        // Davranış özeti (hata senaryoları, sahiplik denetimi, iki yollu koşullar, dolaylı yazma, geri alma), kapının
        // "dosya:satır" anahtarıyla; kapsamı her zaman açık
        "dna": dna_haritasi,
        // Veri şemasındaki ve göç dosyalarındaki kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"]
        "kaskadlar": uygulama_bilgi.get("kaskadlar").cloned().unwrap_or_else(|| json!([])),
        // Yolu çalışma anında kurulan rota kayıtları (Go): "dosya:satır ifade"; haritada yoktur
        "dinamik_kayitlar": uygulama_bilgi.get("dinamik_kayitlar").cloned().unwrap_or_else(|| json!([])),
        // Rota kaynağı bulunup okunamayan yerler ("dosya:satır neden"): bu rotalar haritada yok, kapsam kısmen
        "okunamayan_rotalar": uygulama_bilgi.get("okunamayan_rotalar").cloned().unwrap_or_else(|| json!([])),
        // Dahili olay ve iş düğümleri ("event:X" / "job:X"): dinleyicileri, tetikledikleri, sildikleri; işlem değildir
        "olaylar": olaylar,
        // Dış etki envanteri: giden çağrılar (hedef, güven, ulaşan işlemler), dış sistemler, kuyruk dinleyicileri,
        // model düzeyinde yumuşak silme
        "dis": dis_ozeti,
        "ikincil_kapi": kapilar.iter().filter(|k| !k.ana).count(),
        "dosya": dosya_sayisi,
        "ek_paketler": ek_paketler,
        "satir": satir,
        "endpoint_sayisi": kapilar.len(),
        "kapsam": kapsam_bilgisi,
        "ic_ayrintisi_kesilen": ic_kesilen,
        "govdeli": govdeli,
        "dto_bulunamayan": bulunamayan,
        "veri_semasi": veri_dosyalari,
        "veri_bagli": veri_bagli,
        "buyuk_diye_atlanan": buyuk,
        "kismen_okunan_dosya": kismi,
        // Ön yüz klasörü (uygulama köküne göre): izleme ve bayatlık yeni ön yüz dosyalarını burada da arar
        "on_yuz": on_yuz.as_ref().and_then(|k| kesif::goreli_yol(&kok, k)),
        // Adaptörün okuduğu dosyalar ve içerik özetleri: bayatlık ve paylaşım dosyası bunlara bakar
        "okunan_dosyalar": okunan.iter().filter_map(|(p, o)| Some(json!({ "yol": kesif::goreli_yol(&kok, p)?, "ozet": o }))).collect::<Vec<_>>(),
        "sure_ms": { "kesif": t_kesif.as_millis(), "ayristirma": (t_ayristirma - t_kesif).as_millis(), "adaptor": (t_toplam - t_ayristirma).as_millis(), "toplam": t_toplam.as_millis(), "cagri": t_cagri.as_millis() },
    });
    // Adaptörün haritaya almadığı URL kayıtları ve nedeni (Django yönetim paneli, projenin dışındaki URL yapılandırması,
    // çözülemeyen görünüm): kapsam raporu bunları "bilinmeyen" olarak gösterir
    if let Some(h) = uygulama_bilgi.get("haritalanmayan").filter(|h| h.as_array().is_some_and(|a| !a.is_empty())) {
        ozet["haritalanmayan"] = h.clone();
    }
    // Kapıya çevrilemeyen rota tanımları (dinamik kayıt, Rack uygulaması, eylemi olmayan rota): yalnızca adaptör
    // bildirdiyse yazılır
    if let Some(c) = uygulama_bilgi.get("cozulemeyen_rotalar").filter(|c| c.as_array().is_some_and(|a| !a.is_empty())) {
        ozet["cozulemeyen_rotalar"] = c.clone();
    }
    // Kanıt deposu ve graf (ön yüz bağları dahil) haritadan önce yazılır: haritanın değiştiğini görüp yeniden yükleyen
    // MCP sunucusu ve Ayna yeni kanıtları da bulur; kart kanıtlarını bu depodan alır
    let okunamayan_rotalar = adaptor::okunamayan_rota_kaynaklari(&ozet);
    let mut harita = json!({ "ozet": ozet, "endpointler": kapilar });
    // Dış etkili ya da geri alınamaz yazmanın onay gerekçesi kapıda: canlı modun onay kuralı (statik kısım) okur
    dis::kapilara_yaz(&mut harita["endpointler"], &etki_onaylari);
    dis_zincir::kapilara_yaz(&mut harita["endpointler"], &dis_bilinmeyen);
    // Adaptörün yan bilgisi (modül ve router grafiği, tablo modelleri) haritaya yazılmaz, yalnızca kanıt deposuna girer
    let depo = kanit_yaz(&harita, web_baglari.as_ref().unwrap_or(&serde_json::Value::Null), &uygulama_bilgi, on_yuz.as_deref(), &kok, &cikti);
    // Atomik yazım: çalışan bir MCP sunucusu ya da Ayna hiçbir zaman yarım dosya okumaz
    if let Err(e) = guvenli::atomik_yaz(&cikti.join("kapilar.json"), serde_json::to_string_pretty(&harita).unwrap().as_bytes()) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
    // Bütünlük kontrolü: yazılan harita okunup bellektekiyle karşılaştırılır; Sezgi sessizce eksik haritayla çalışmaz
    let yazilan = guvenli::oku_harita(&cikti.join("kapilar.json")).and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok());
    if yazilan.as_ref().and_then(|v| v["endpointler"].as_array().map(|a| a.len())) != Some(kapilar.len()) {
        let boy = std::fs::metadata(cikti.join("kapilar.json")).map(|m| m.len()).unwrap_or(0);
        if boy > guvenli::HARITA_SINIRI {
            te!(
                "✗ Harita {} MB: okunabilecek en büyük harita {} MB. Lütfen bunu bildirin (uygulamanın adı ve işlem sayısıyla); Sezgi bu boyutta bir haritayı okuyamaz",
                "✗ The map is {} MB: the largest map Sezgi can read is {} MB. Please report this (with the application's name and number of operations); Sezgi cannot read a map this size",
                boy >> 20,
                guvenli::HARITA_SINIRI >> 20
            );
            std::process::exit(1);
        }
        te!(
            "✗ Harita eksik yazıldı ya da okunamıyor ({}): disk dolu ya da başka bir süreç dosyayı değiştiriyor olabilir",
            "✗ The map was written incompletely or cannot be read ({}): the disk may be full or another process may be changing the file",
            cikti.join("kapilar.json").display()
        );
        std::process::exit(1);
    }
    let verilis = simdi_sn();
    let mut kart = benlik::kart(&ad, &uygulama_surumu, &ref_, &adaptor_kimligi, &kapilar, &benlik::zaman_metni(verilis));
    // Kart haritaya bağlanır: canlı mod onay kararını haritadan okur, harita sonradan değişirse yazma yapılmaz
    kart[benlik::HARITA_OZETI] = serde_json::json!(benlik::harita_ozeti(&harita));
    benlik::dna_ekle(&mut kart, &ozet["dna"]);
    benlik::olaylari_ekle(&mut kart, &ozet["dna"], &ozet["olaylar"]);
    benlik::dis_ekle(&mut kart, &ozet["dna"], &ozet["dis"]);
    // Okunamayan rota kaynakları: kartın işlem listesi eksik (`x-unread_route_sources`)
    benlik::okunamayan_rotalari_ekle(&mut kart, &okunamayan_rotalar);
    // Uygulamanın anlamı (alan, amaç alıntısı, kullanıcı tipleri, iş yetenekleri): `app.x-meaning`, en fazla inferred
    anlam::kart_ekle(&mut kart, &depo);
    // 0.1.1 alanları: geçerlilik süresi, dayanak özetleri (bayatlık bunlardan hesaplanır), kanıt özetleri ve paketi
    let mut kanit_paketi = benlik::tamamla(&mut kart, &kapilar, &depo, &kok, verilis, gecerlilik);
    // Veri defteri: bu bilgisayarda canlı mod kullanıldıysa kart işletim kaydının tutulduğunu ve süresini söyler
    if let Some(d) = canli::kart_defteri(&cikti) {
        kart["data_ledger"] = d;
    }
    // Önceki karttan bu yana değişenler (SPEC §7): işlemler değiştiyse hesaplanır (commit gerekmez), değişmediyse önceki fark korunur
    let onceki: Option<serde_json::Value> = guvenli::oku(&cikti.join("benlik.json")).and_then(|m| serde_json::from_str(&m).ok());
    if let Some(c) = onceki.as_ref().and_then(|o| fark::kart_degisiklikleri(o, &kart)) {
        let imza = kart.as_object_mut().unwrap().remove("signature");
        kart["changes"] = c;
        if let Some(i) = imza {
            kart["signature"] = i;
        }
    }
    // --imzala [anahtar-adı]: kartı ML-DSA-65 ile imzalar
    let imza_durumu = if let Some(i) = args.iter().position(|a| a == "--imzala") {
        let kimlik = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "varsayilan".into());
        // Kanıt paketi kartla aynı anahtarla, kendi bağlamıyla imzalanır: paket imzası kart imzası yerine geçemez
        match imza::imzala(&mut kart, &kimlik).and_then(|_| imza::imzala_baglamla(&mut kanit_paketi, &kimlik, imza::KANIT_BAGLAMI)) {
            Ok(()) => {
                // Açık anahtar kartın yanına kopyalanamazsa kart doğrulanamaz: başarı sayılmaz
                if let Err(e) = std::fs::copy(imza::acik_anahtar_dosyasi(&kimlik), cikti.join("benlik-keys.json")) {
                    te!("✗ Açık anahtar belgesi kopyalanamadı ({e}): {}", "✗ Could not copy the public key document ({e}): {}", imza::acik_anahtar_dosyasi(&kimlik).display());
                    std::process::exit(1);
                }
                tf!("ML-DSA-65 ile imzalandı (anahtar '{kimlik}')", "signed with ML-DSA-65 (key '{kimlik}')")
            }
            Err(e) => {
                te!("✗ İmzalanamadı: {e}", "✗ Could not sign: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // Önceki imzalı kurulumdan kalan açık anahtar imzasız kartın yanında durmasın (bağ değilse silinir; bağ ise
        // yukarıda yazma zaten reddedildi)
        let eski = cikti.join("benlik-keys.json");
        if std::fs::symlink_metadata(&eski).map(|m| m.file_type().is_file()).unwrap_or(false) {
            if let Err(e) = std::fs::remove_file(&eski) {
                te!("✗ Eski açık anahtar belgesi silinemedi ({e}): {}", "✗ Could not delete the old public key document ({e}): {}", eski.display());
                std::process::exit(1);
            }
        }
        tf!("imzasız (imzalamak için: {})", "unsigned (to sign: {})", imza::imzalama_ipucu("sezgi init"))
    };
    if let Err(e) = guvenli::atomik_yaz(&cikti.join(benlik::KANIT_PAKETI), serde_json::to_string_pretty(&kanit_paketi).unwrap().as_bytes()) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
    if let Err(e) = guvenli::atomik_yaz(&cikti.join("benlik.json"), serde_json::to_string_pretty(&kart).unwrap().as_bytes()) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
    // Uygulamanın güncel OpenAPI belgesi: elle yazılmış doküman yoksa bile kendiliğinden oluşur
    let belge = openapi::uret(&kapilar, &ad, &uygulama_surumu, &ozet);
    if let Err(e) = guvenli::atomik_yaz(&cikti.join("openapi.json"), serde_json::to_string_pretty(&belge).unwrap().as_bytes()) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
    // Ön yüz bağları: ekran → UI eylemi → istemci çağrısı → bu haritanın uçları (web-baglari.json)
    let web_dosyasi = cikti.join("web-baglari.json");
    let web_ozet = match web_baglari {
        Some(w) => {
            if let Err(e) = guvenli::atomik_yaz(&web_dosyasi, serde_json::to_string_pretty(&w).unwrap().as_bytes()) {
                eprintln!("✗ {e}");
                std::process::exit(1);
            }
            Some(w["istatistik"].clone())
        }
        None => {
            // Önceki kurulumdan kalan bağlar ön yüz artık yokken durmasın
            if std::fs::symlink_metadata(&web_dosyasi).map(|m| m.file_type().is_file()).unwrap_or(false) {
                let _ = std::fs::remove_file(&web_dosyasi);
            }
            None
        }
    };
    let sonuc = InitOzet { endpoint: kapilar.len(), dosya: dosya_toplam, ayristirilan, sure: t0.elapsed() };
    if !ayrintili {
        return Some(sonuc);
    }

    // Kısa, iç terimsiz özet (en çok 12 satır): ne okundu, ne bulundu, neyin eksik kaldığı ve sonraki adım
    let rol = kapilar.iter().filter(|k| !k.roller.is_empty()).count();
    let acik = kapilar.iter().filter(|k| k.giris == "herkese_acik").count();
    let girisli = kapilar.iter().filter(|k| k.giris == "gerekli").count();
    println!("◆ Sezgi {} · {}", env!("CARGO_PKG_VERSION"), ad);
    let kardes = if ek_paketler.is_empty() { String::new() } else { tf!(" · kardeş paket: {}", " · sibling packages: {}", ek_paketler.join(", ")) };
    tp!("  Okundu       {} dosya · {} satır{kardes}", "  Read         {} {f} · {} {l}{kardes}", dosya_sayisi, satir; f = crate::dil::cogul(dosya_sayisi, "file", "files"), l = crate::dil::cogul(satir, "line", "lines"));
    let mut islem =
        tf!("{} · {girisli} giriş ister · {acik} girişsiz", "{} · {girisli} {g} login · {acik} without login", kapilar.len(); g = crate::dil::cogul(girisli, "needs", "need"));
    if rol > 0 {
        islem.push_str(&tf!(" · {rol} rol kısıtlı", " · {rol} role-restricted"));
    }
    tp!("  İşlemler     {islem}", "  Operations   {islem}");
    if !veri_dosyalari.is_empty() {
        tp!("  Tablolar     {} / {} işlem bir tabloya bağlı", "  Tables       {} / {} {i} linked to a table", veri_bagli, kapilar.len(); i = crate::dil::cogul(kapilar.len(), "operation", "operations"));
    }
    let mut eksik = vec![];
    if !bulunamayan.is_empty() {
        eksik.push(tf!("{} girdi tanımı bulunamadı", "{} input definitions not found", bulunamayan.len()));
    }
    if !kismi.is_empty() {
        eksik.push(tf!("{} dosya kısmen okundu", "{} {f} partly read", kismi.len(); f = crate::dil::cogul(kismi.len(), "file", "files")));
    }
    if !buyuk.is_empty() {
        eksik.push(tf!("{} dosya 4 MiB'tan büyük, okunmadı", "{} {f} larger than 4 MiB, not read", buyuk.len(); f = crate::dil::cogul(buyuk.len(), "file", "files")));
    }
    if let Some(u) = adaptor::okunamayan_rota_uyarisi(&okunamayan_rotalar) {
        eksik.push(u);
    }
    if !eksik.is_empty() {
        tp!("  Okunamayan   {} → sezgi rapor", "  Unread       {} → sezgi report", eksik.join(" · "));
    }
    for u in kapsam::uyarilar(&harita["ozet"], crate::dil::en()) {
        tp!("  ⚠ Kapsam     {u}", "  ⚠ Coverage   {u}");
    }
    if let Some(i) = &web_ozet {
        tp!(
            "  Ön yüz       {} ekran · {} eylem · {} / {} istemci çağrısı bir işleme bağlı · {} ms",
            "  Front end    {} {e} · {} {a} · {} / {} client {c} linked to an operation · {} ms",
            i["ekran"],
            i["eylem"],
            i["uca_baglanan"],
            i["istemci_cagrisi"],
            i["sure_ms"];
            e = crate::dil::cogul(&i["ekran"], "screen", "screens"),
            a = crate::dil::cogul(&i["eylem"], "action", "actions"),
            c = crate::dil::cogul(&i["istemci_cagrisi"], "call", "calls")
        );
    }
    tp!("  Kart         {} · {gecerlilik} gün geçerli", "  Card         {} · valid for {gecerlilik} {g}", imza_durumu; g = crate::dil::cogul(gecerlilik, "day", "days"));
    // Harita yolu bir kez: bulunulan klasörün altındaysa göreli
    let calisma = std::env::current_dir().ok().and_then(|c| std::fs::canonicalize(c).ok());
    let mutlak = std::fs::canonicalize(&cikti).unwrap_or_else(|_| cikti.clone());
    let goreli = calisma.as_ref().and_then(|c| mutlak.strip_prefix(c).ok()).map(|p| p.display().to_string());
    let varsayilan = goreli.as_deref() == Some(".sezgi");
    let gorunen = goreli.unwrap_or_else(|| guvenli::gorunen(&cikti));
    tp!("  Süre         {} ms · harita: {gorunen}", "  Time         {} ms · map: {gorunen}", t_toplam.as_millis());
    println!();
    tp!("Sonraki adım:", "Next step:");
    // Varsayılan yerde değilse komutlar haritanın yolunu ister; `izle` uygulama klasörünü (ve özel çıktıyı) ister:
    // kökten çalıştırılan `sezgi izle` alt klasördeki uygulamayı bulamaz
    let ek = if varsayilan { String::new() } else { format!(" {gorunen}") };
    let uygulama_goreli = calisma.as_ref().and_then(|c| kok.strip_prefix(c).ok()).map(|p| p.display().to_string()).unwrap_or_else(|| guvenli::gorunen(&kok));
    let mut izle = t!("sezgi izle", "sezgi watch").to_string();
    if !uygulama_goreli.is_empty() {
        izle.push_str(&format!(" {uygulama_goreli}"));
    }
    if mutlak != kok.join(".sezgi") {
        izle.push_str(&tf!(" --cikti {gorunen}", " --out {gorunen}"));
    }
    let satirlar = [
        (tf!("sezgi bagla{ek}", "sezgi connect{ek}"), t!("yapay zekâ aracınıza bağlar (elle: sezgi bagla --elle)", "connects your AI tool (by hand: sezgi connect --manual)")),
        (tf!("sezgi ayna{ek}", "sezgi mirror{ek}"), t!("ne bildiğini tarayıcıda gösterir", "shows what it knows in your browser")),
        (izle, t!("kod değiştikçe güncel tutar", "keeps it up to date as the code changes")),
    ];
    // Komut sütunu en az 21 karakter, en uzun komuttan sonra iki boşluk: açıklamalar aynı sütunda başlar
    let genislik = satirlar.iter().map(|(k, _)| k.chars().count() + 2).max().unwrap_or(0).max(21);
    for (k, a) in &satirlar {
        println!("  {k:<genislik$}{a}");
    }
    Some(sonuc)
}
