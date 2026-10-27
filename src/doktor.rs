//! `sezgi doktor [uygulama-klasörü=.] [--harita <klasör>] [--rea] [--json]`: neyin eksik ya da bozuk olduğunu söyler ve
//! nasıl düzeltileceğini yazar. Salt okunurdur: hiçbir dosya yazmaz, görülen anahtarları bile kaydetmez.
//! Kaynaksız uygulamanın klasöründe (`sezgi kesfet` çıktısı) çatı ve kaynak kodu aranmaz; yapıtların değişip
//! değişmediği ve REA köprüsünün durumu denetlenir. REA durumu yalnızca `--rea` ile ya da kaynaksız klasörde bakılır;
//! ağa çıkılmaz.
//! Hata varsa çıkış kodu 1'dir; uyarılar (ör. bayat harita) başarısız sayılmaz.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::{baglan, imza};

struct Kontrol {
    ad: String,
    baslik: String,
    durum: &'static str,
    mesaj: String,
}

fn k(ad: &str, baslik: &str, durum: &'static str, mesaj: impl Into<String>) -> Kontrol {
    Kontrol { ad: ad.into(), baslik: baslik.into(), durum, mesaj: mesaj.into() }
}

/// Kart gerçekten imzalı mı (imzasız kartta da "signature" alanı vardır: alg "none")
fn imzali(kart: &Value) -> bool {
    kart["signature"]["alg"].as_str().is_some_and(|a| a != "none") && kart["signature"]["value"].as_str() != Some("UNSIGNED-DEV-BUILD")
}

fn json_dosya(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

pub fn calistir(args: &[String]) {
    let proje_arg = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let proje = std::fs::canonicalize(&proje_arg).map(|p| PathBuf::from(p.display().to_string().trim_start_matches(r"\\?\"))).unwrap_or(proje_arg);
    let harita = args.iter().position(|a| a == "--harita").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(|| proje.join(".sezgi"));
    let mut sonuc: Vec<Kontrol> = vec![];

    // Sezgi'nin kendisi
    let exe = std::env::current_exe().ok();
    let npx = exe.as_ref().is_some_and(|e| e.components().any(|c| c.as_os_str() == "_npx"));
    sonuc.push(if npx {
        k(
            "surum",
            "Sezgi",
            "uyari",
            tf!(
                "{} npx önbelleğinden çalışıyor; önbellek temizlenirse bağlantılar bozulur → kalıcı kurulum: npm install -g sezgi",
                "{} runs from the npx cache; connections break when the cache is cleaned → permanent install: npm install -g sezgi",
                env!("CARGO_PKG_VERSION")
            ),
        )
    } else {
        k("surum", "Sezgi", "tamam", env!("CARGO_PKG_VERSION"))
    });

    let kaynaksiz = json_dosya(&harita.join("kapilar.json")).is_some_and(|h| h["ozet"]["kaynaksiz"] == true);
    if kaynaksiz {
        kaynaksiz_kontroller(&harita, &mut sonuc);
    } else {
        kaynakli_kontroller(&proje, &harita, &mut sonuc);
    }
    // REA köprüsü: yalnızca istenince ya da kaynaksız uygulamada
    let rea_iste = args.iter().any(|a| a == "--rea");
    if rea_iste || kaynaksiz {
        let (durum, mesaj) = crate::kesif::rea::durum();
        sonuc.push(match durum {
            "hazir" => k("rea", "REA", "tamam", mesaj),
            crate::kesif::rea::YOK if !rea_iste => {
                k("rea", "REA", "bilgi", tf!("kurulu değil (isteğe bağlı; .NET üyeleri için): {mesaj}", "not installed (optional; for .NET members): {mesaj}"))
            }
            crate::kesif::rea::YOK => k("rea", "REA", "uyari", tf!("kurulu değil: {mesaj}", "not installed: {mesaj}")),
            t => {
                let tur = match t {
                    crate::kesif::rea::SURUM_UYUSMUYOR => t!("sürüm uyuşmuyor", "version mismatch"),
                    crate::kesif::rea::SABITLEME => t!("sabitleme hatası", "pinning error"),
                    _ => t!("çalışmadı", "did not run"),
                };
                k(
                    "rea",
                    "REA",
                    if rea_iste { "hata" } else { "uyari" },
                    tf!("{tur}: {mesaj} (köprü kapalı kalır) → yeniden kurmak için: sezgi rea kur", "{tur}: {mesaj} (the bridge stays off) → to reinstall: sezgi rea install"),
                )
            }
        });
    }

    // Benlik Kartı ve imza (salt okunur: görülen anahtar kaydedilmez)
    benlik_kontrolu(&harita, &mut sonuc, if kaynaksiz { t!("sezgi kesfet <klasör>", "sezgi discover <folder>") } else { "sezgi init" });

    // Canlı hedef: gerçek veride yazma açıksa açıkça söylenir (salt okunur: durum kurulmadıysa hiçbir şey yazılmaz)
    if let Some(d) = crate::canli::Durum::bul(&harita) {
        let ayar = d.ayar_oku();
        if let Some(url) = ayar["hedef"]["url"].as_str() {
            let url = crate::guvenli::temiz(url);
            sonuc.push(if d.gercek_veri() {
                match d.gercek_yazma_acik() {
                    Ok(()) => k(
                        "canli",
                        t!("Canlı hedef", "Live target"),
                        "uyari",
                        tf!(
                            "GERÇEK VERİ: {url} için yazma açık (deneysel; mod {}) · her yazma terminalde insan parolasıyla onay ister · saatte en çok {} yazma (son saatte {}) · parolayı bilen ya da yönetici yetkisiyle çalışan programı ve kabuk erişimiyle oturum belirtecini okuyup uygulamaya doğrudan giden ajanı durdurmaz → kapatmak: sezgi hedef {url}", "REAL DATA: writing is on for {url} (experimental; mode {}) · every write needs approval with the human password in a terminal · at most {} writes per hour ({} in the last hour) · it does not stop a program that knows the password or runs with administrator rights, nor an agent with shell access that reads the session token and calls the application directly → to turn off: sezgi target {url}",
                            d.mod_(),
                            d.saatlik_yazma(),
                            d.son_saat_yazma()
                        ),
                    ),
                    Err(e) => k("canli", t!("Canlı hedef", "Live target"), "uyari", tf!("GERÇEK VERİ: {url} bildirilmiş ama yazma kapalı (deneysel): {e}", "REAL DATA: {url} is declared but writing is off (experimental): {e}")),
                }
            } else if ayar["hedef"]["test_kopyasi"] == true {
                match d.test_kopyasi_yazilabilir() {
                    Ok(()) => k("canli", t!("Canlı hedef", "Live target"), "bilgi", tf!("{url} (test kopyası, mod {})", "{url} (test copy, mode {})", d.mod_())),
                    Err(e) => k("canli", t!("Canlı hedef", "Live target"), "uyari", tf!("{url} (test kopyası, mod {}): {e}", "{url} (test copy, mode {}): {e}", d.mod_())),
                }
            } else {
                k("canli", t!("Canlı hedef", "Live target"), "bilgi", tf!("{url} (yalnızca okuma, mod {}; gerçek veride yazma kapalı)", "{url} (read only, mode {}; writing on real data is off)", d.mod_()))
            });
            // İnsan kanıtı: sistem çapası (salt okunur)
            sonuc.push(match crate::insan::capa() {
                crate::insan::CapaDurumu::Var(c) => k(
                    "insan",
                    t!("İnsan anahtarı", "Human key"),
                    "tamam",
                    tf!("sistem çapası {} · iz {} · kuruldu {}", "system anchor {} · fingerprint {} · set up {}", crate::guvenli::gorunen(&c.yol), c.izi, c.kuruldu),
                ),
                crate::insan::CapaDurumu::Gecersiz(y, e) => k(
                    "insan",
                    t!("İnsan anahtarı", "Human key"),
                    "uyari",
                    tf!(
                        "sistem çapası geçersiz ({}: {e}): gerçek veride yazma kapalı → {}",
                        "system anchor invalid ({}: {e}): writing on real data is off → {}",
                        crate::guvenli::gorunen(&y),
                        crate::insan::kur_komutu()
                    ),
                ),
                crate::insan::CapaDurumu::Yok(_) => k(
                    "insan",
                    t!("İnsan anahtarı", "Human key"),
                    "bilgi",
                    tf!(
                        "sistem çapası yok: gerçek veride yazma kapalı, test kopyasında insan kanıtı yalnızca terminal denetimi → {}",
                        "no system anchor: writing on real data is off; on a test copy the only human proof is the terminal check → {}",
                        crate::insan::kur_komutu()
                    ),
                ),
            });
        }
    }

    // Yapay zekâ bağlantıları
    let baglantilar = baglan::baglanti_durumlari(&proje);
    if baglantilar.is_empty() {
        sonuc.push(k(
            "baglanti",
            t!("Bağlantı", "Connection"),
            "bilgi",
            t!("Sezgi'nin eklediği bağlantı yok (isteğe bağlı: sezgi bagla)", "no connection added by Sezgi (optional: sezgi connect)"),
        ));
    }
    for (arac, _, d) in baglantilar {
        let ad = format!("baglanti:{arac}");
        let baslik = tf!("Bağlantı {arac}", "Connection {arac}");
        sonuc.push(match d {
            baglan::Durum::Tamam(m) => k(&ad, &baslik, "tamam", m),
            baglan::Durum::Uyari(m) => k(&ad, &baslik, "uyari", m),
            baglan::Durum::Hata(m) => k(&ad, &baslik, "hata", m),
        });
    }
    rapor(args, &proje, sonuc);
}

/// Kaynaksız uygulama (`sezgi kesfet`): kanıt deposu ve yapıtların bugünkü özetleri
fn kaynaksiz_kontroller(harita: &Path, sonuc: &mut Vec<Kontrol>) {
    let h = json_dosya(&harita.join("kapilar.json")).unwrap_or_default();
    let ozet = &h["ozet"];
    sonuc.push(k(
        "uygulama",
        t!("Uygulama", "Application"),
        "tamam",
        tf!(
            "{} (kaynaksız: {}; kaynak kodu yok, işlem yok)",
            "{} (without source: {}; no source code, no operations)",
            ozet["uygulama"].as_str().unwrap_or("?"),
            ozet["uygulama_dugumu"].as_str().unwrap_or("?")
        ),
    ));
    match crate::kanit::Model::yukle(harita) {
        None => sonuc.push(k(
            "harita",
            t!("Kanıt deposu", "Evidence store"),
            "hata",
            tf!("{} okunamıyor → sezgi kesfet <klasör>", "{} cannot be read → sezgi discover <folder>", harita.join(crate::kanit::DOSYA).display()),
        )),
        Some(m) => {
            sonuc.push(k(
                "harita",
                t!("Kanıt deposu", "Evidence store"),
                "tamam",
                tf!("{} kanıt, {} önerme, {} düğüm", "{} evidence {k}, {} {o}, {} {d}", m.depo.kanitlar.len(), m.depo.onermeler.len(), m.graf.dugumler.len(); k = crate::dil::cogul(m.depo.kanitlar.len(), "record", "records"), o = crate::dil::cogul(m.depo.onermeler.len(), "claim", "claims"), d = crate::dil::cogul(m.graf.dugumler.len(), "node", "nodes")),
            ));
            let eski: Vec<&str> = m.depo.kanitlar.iter().filter(|x| !x.gecerli).filter_map(|x| x.dayanak.first().map(|d| d.yol.as_str())).collect();
            let mut eski: Vec<&str> = eski.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            eski.truncate(5);
            if m.depo.kok.is_none() {
                sonuc.push(k(
                    "tazelik",
                    t!("Tazelik", "Freshness"),
                    "bilgi",
                    t!("çıktı yapıt klasörünün dışında: yapıtların değişimi izlenmiyor", "the output is outside the artifact folder: changes to the artifacts are not tracked"),
                ));
            } else if eski.is_empty() {
                sonuc.push(k("tazelik", t!("Tazelik", "Freshness"), "tamam", t!("yapıtlar keşiften bu yana değişmedi", "the artifacts have not changed since discovery")));
            } else {
                sonuc.push(k(
                    "tazelik",
                    t!("Tazelik", "Freshness"),
                    "uyari",
                    tf!("yapıt değişti ya da okunamıyor ({}) → sezgi kesfet <klasör>", "an artifact changed or cannot be read ({}) → sezgi discover <folder>", eski.join(", ")),
                ));
            }
        }
    }
}

/// Kaynak kodlu uygulama: çatı, harita ve tazelik
fn kaynakli_kontroller(proje: &Path, harita: &Path, sonuc: &mut Vec<Kontrol>) {
    // Uygulama ve çatı: init'in seçimiyle aynı (crate::cati_sec)
    let kurallar: crate::adaptor::Kurallar = toml::from_str(crate::NESTJS).expect("nestjs.toml");
    let pkg = crate::guvenli::oku(&proje.join("package.json")).unwrap_or_default();
    let secim = crate::cati_sec(proje, &pkg, &kurallar);
    let adaptor_kimligi = secim.map(|s| crate::adaptor_kimligi(s, &kurallar)).unwrap_or_default();
    // package.json'ı olmayan çatılar: uygulamanın adı kendi tanım dosyasından (pyproject.toml, pom.xml / Gradle, Gemfile, .csproj, go.mod, composer.json)
    let python = secim.is_some_and(|s| {
        s.python_mi() || matches!(s, crate::Secim::Spring | crate::Secim::Ktor | crate::Secim::Rails | crate::Secim::AspNet | crate::Secim::Go | crate::Secim::Laravel)
    });
    if python {
        let p = match secim {
            Some(crate::Secim::Spring) => crate::spring::paket(proje),
            Some(crate::Secim::Ktor) => crate::ktor::paket(proje),
            Some(crate::Secim::Rails) => crate::rails::paket(proje),
            Some(crate::Secim::AspNet) => crate::aspnet::paket(proje),
            Some(crate::Secim::Go) => crate::golang::paket(proje),
            Some(crate::Secim::Laravel) => crate::laravel::paket(proje),
            _ => crate::python::paket(proje),
        };
        sonuc.push(k("uygulama", t!("Uygulama", "Application"), "tamam", format!("{} ({})", p["name"].as_str().unwrap_or(t!("adsız", "unnamed")), proje.display())));
    }
    match json_dosya(&proje.join("package.json")).filter(|_| !python) {
        Some(p) => sonuc.push(k("uygulama", t!("Uygulama", "Application"), "tamam", format!("{} ({})", p["name"].as_str().unwrap_or(t!("adsız", "unnamed")), proje.display()))),
        None if python => {}
        None => sonuc.push(k(
            "uygulama",
            t!("Uygulama", "Application"),
            "hata",
            tf!(
                "{} içinde Sezgi'nin okuyabildiği bir uygulama yok (desteklenenler: {}) → uygulama klasöründe çalıştırın ya da yolunu verin: sezgi doktor <klasör>; kaynak kodu olmayan uygulama (.exe / .dll, APK, SQLite veritabanı) için: sezgi kesfet <klasör|dosya>", "{} has no application Sezgi can read (supported: {}) → run it in the application folder or give its path: sezgi doctor <folder>; for an application without source code (.exe / .dll, APK, SQLite database): sezgi discover <folder|file>",
                proje.display(),
                crate::DESTEKLENEN_CATILAR
            ),
        )),
    }
    let isaret = &kurallar.adaptor.isaret_bagimlilik;
    sonuc.push(match secim {
        Some(crate::Secim::NestJs) => k("cati", t!("Çatı", "Framework"), "tamam", format!("NestJS ({isaret})")),
        Some(crate::Secim::FastApi) => k("cati", t!("Çatı", "Framework"), "tamam", "FastAPI (Python)"),
        Some(crate::Secim::Django) => k("cati", t!("Çatı", "Framework"), "tamam", t!("Django (Python; Django REST Framework görünümleri, router'ları ve serializer'ları dahil)", "Django (Python; including Django REST Framework views, routers and serializers)")),
        Some(crate::Secim::Flask) => k("cati", t!("Çatı", "Framework"), "tamam", t!("Flask (Python; Blueprint, MethodView, Flask-RESTful / Flask-Smorest / Flask-RESTX dahil)", "Flask (Python; including Blueprint, MethodView, Flask-RESTful / Flask-Smorest / Flask-RESTX)")),
        Some(crate::Secim::Rails) => k("cati", t!("Çatı", "Framework"), "tamam", "Ruby on Rails (Gemfile, config/routes.rb)"),
        Some(crate::Secim::Laravel) => k("cati", t!("Çatı", "Framework"), "tamam", "Laravel (PHP)"),
        Some(crate::Secim::Express) => k("cati", t!("Çatı", "Framework"), "tamam", format!("{} (JavaScript / TypeScript)", crate::cati(proje).unwrap_or("Express"))),
        Some(crate::Secim::Spring) => k("cati", t!("Çatı", "Framework"), "tamam", if crate::spring::kotlin_mi(proje) { "Spring Boot (Kotlin)" } else { "Spring Boot (Java)" }),
        Some(crate::Secim::Ktor) => k("cati", t!("Çatı", "Framework"), "tamam", "Ktor (Kotlin)"),
        Some(crate::Secim::AspNet) => k("cati", t!("Çatı", "Framework"), "tamam", "ASP.NET Core (C#)"),
        Some(crate::Secim::Go) => k("cati", t!("Çatı", "Framework"), "tamam", format!("{} (Go, go.mod)", crate::golang::cati_adi(proje))),
        None => k(
            "cati",
            t!("Çatı", "Framework"),
            "hata",
            tf!("desteklenen bir çatı bulunamadı: package.json'da {isaret}, express, fastify ya da next; pyproject.toml / requirements.txt'te fastapi, django ya da flask; pom.xml / build.gradle'da spring-boot ya da io.ktor; Gemfile'da rails; .csproj'ta Microsoft.NET.Sdk.Web; go.mod; composer.json'da laravel/framework", "no supported framework found: {isaret}, express, fastify or next in package.json; fastapi, django or flask in pyproject.toml / requirements.txt; spring-boot or io.ktor in pom.xml / build.gradle; rails in Gemfile; Microsoft.NET.Sdk.Web in a .csproj; go.mod; laravel/framework in composer.json"),
        ),
    });

    // Harita
    let kapilar_yolu = harita.join("kapilar.json");
    match json_dosya(&kapilar_yolu) {
        None => sonuc.push(k(
            "harita",
            t!("Harita", "Map"),
            "hata",
            tf!("{} yok ya da okunamıyor → sezgi init", "{} is missing or cannot be read → sezgi init", kapilar_yolu.display()),
        )),
        Some(h) => {
            let ozet = &h["ozet"];
            let n = h["endpointler"].as_array().map(|a| a.len()).unwrap_or(0);
            let uretici = ozet["uretici"].as_str().unwrap_or("?").to_string();
            sonuc.push(if n == 0 {
                k(
                    "harita",
                    t!("Harita", "Map"),
                    "uyari",
                    t!(
                        "hiç işlem bulunamadı → sezgi rapor ile kapsamı görün; controller'lar başka bir klasördeyse o klasörü verin",
                        "no operations found → see the coverage with sezgi report; if the controllers are in another folder, give that folder"
                    ),
                )
            } else if let Some(u) = crate::adaptor::okunamayan_rota_uyarisi(&crate::adaptor::okunamayan_rota_kaynaklari(ozet)) {
                // Rota kaynağı bulunup okunamayan yer: işlem listesi eksik, kapsam "tam" değil
                k(
                    "harita",
                    t!("Harita", "Map"),
                    "uyari",
                    tf!("{n} işlem, kapsam kısmen: {u} → sezgi rapor", "{n} {i}, coverage partial: {u} → sezgi report"; i = crate::dil::cogul(n, "operation", "operations")),
                )
            } else {
                k(
                    "harita",
                    t!("Harita", "Map"),
                    "tamam",
                    tf!("{n} işlem ({uretici}, {})", "{n} {i} ({uretici}, {})", ozet["adaptor"].as_str().unwrap_or("?"); i = crate::dil::cogul(n, "operation", "operations")),
                )
            });
            let simdiki = format!("sezgi {}", env!("CARGO_PKG_VERSION"));
            if uretici != simdiki || ozet["adaptor"].as_str() != Some(adaptor_kimligi.as_str()) {
                sonuc.push(k(
                    "uretici",
                    t!("Sürüm", "Version"),
                    "uyari",
                    tf!(
                        "harita {uretici} / {} ile üretildi, şimdiki {simdiki} / {adaptor_kimligi} → sezgi init",
                        "the map was built with {uretici} / {}, current is {simdiki} / {adaptor_kimligi} → sezgi init",
                        ozet["adaptor"].as_str().unwrap_or("?")
                    ),
                ));
            }
            // Tazelik: git commit'i ve haritadan sonra değişen dosyalar
            let eski_ref = ozet["kaynak_ref"].as_str().unwrap_or("unknown").to_string();
            let yeni_ref = crate::kaynak_ref(proje);
            // İçerik özetiyle (okunan dosyalar) ve yeni kaynak dosyalar: bayatlık kapısıyla aynı karar
            let degisen = crate::izle::degisen_dosyalar(proje, harita);
            if eski_ref != "unknown" && yeni_ref != "unknown" && eski_ref != yeni_ref {
                sonuc.push(k(
                    "tazelik",
                    t!("Tazelik", "Freshness"),
                    "uyari",
                    tf!("harita {eski_ref} commit'inden, kod şimdi {yeni_ref} → sezgi init", "the map is from commit {eski_ref}, the code is now at {yeni_ref} → sezgi init"),
                ));
            } else if !degisen.is_empty() {
                let ornek = degisen.iter().take(3).map(|p| p.strip_prefix(proje).unwrap_or(p).display().to_string()).collect::<Vec<_>>().join(", ");
                sonuc.push(k(
                    "tazelik",
                    t!("Tazelik", "Freshness"),
                    "uyari",
                    tf!("{} dosya haritadan sonra değişti ({ornek}) → sezgi init", "{} {f} changed since the map was built ({ornek}) → sezgi init", degisen.len(); f = crate::dil::cogul(degisen.len(), "file", "files")),
                ));
            } else {
                sonuc.push(k("tazelik", t!("Tazelik", "Freshness"), "tamam", t!("harita koddan yeni", "the map is up to date with the code")));
            }
        }
    }
}

/// Benlik Kartı ve imza (salt okunur: görülen anahtar kaydedilmez)
fn benlik_kontrolu(harita: &Path, sonuc: &mut Vec<Kontrol>, uret: &str) {
    if harita.is_dir() {
        match json_dosya(&harita.join("benlik.json")) {
            None => sonuc.push(k(
                "kart",
                t!("Benlik Kartı", "Benlik Card"),
                "uyari",
                tf!("benlik.json yok ya da okunamıyor → {uret}", "benlik.json is missing or cannot be read → {uret}"),
            )),
            Some(kart) => match json_dosya(&harita.join("benlik-keys.json")) {
                None if imzali(&kart) => sonuc.push(k(
                    "kart",
                    t!("Benlik Kartı", "Benlik Card"),
                    "uyari",
                    tf!(
                        "kart imzalı ama benlik-keys.json yok: imza doğrulanamaz → {uret} --imzala",
                        "the card is signed but benlik-keys.json is missing: the signature cannot be verified → {uret} --sign"
                    ),
                )),
                None if crate::canli::Durum::bul(harita).is_some_and(|d| d.gercek_veri()) => sonuc.push(k(
                    "kart",
                    t!("Benlik Kartı", "Benlik Card"),
                    "uyari",
                    tf!(
                        "imzasız: gerçek veride yazma açık ve imzalı kart ister, yazmalar reddedilir → sezgi anahtar olustur; {uret} --imzala",
                        "unsigned: writing on real data is on and needs a signed card, writes are refused → sezgi key create; {uret} --sign"
                    ),
                )),
                None => sonuc.push(k(
                    "kart",
                    t!("Benlik Kartı", "Benlik Card"),
                    "tamam",
                    tf!("imzasız (isteğe bağlı: {uret} --imzala <anahtar>)", "unsigned (optional: {uret} --sign <key>)"),
                )),
                Some(anahtarlar) => match imza::dogrula(&kart, &anahtarlar) {
                    Err(e) => {
                        sonuc.push(k("kart", t!("Benlik Kartı", "Benlik Card"), "hata", tf!("imza geçersiz: {e} → {uret} --imzala", "signature invalid: {e} → {uret} --sign")))
                    }
                    Ok(a) => sonuc.push(match imza::guven_durumu(&kart, &anahtarlar) {
                        imza::Guven::Sabit => {
                            k("kart", t!("Benlik Kartı", "Benlik Card"), "tamam", tf!("imza geçerli, sabitlenmiş anahtar '{a}'", "signature valid, pinned key '{a}'"))
                        }
                        imza::Guven::SabitUyusmaz => k(
                            "kart",
                            t!("Benlik Kartı", "Benlik Card"),
                            "hata",
                            tf!("anahtar '{a}' bu uygulama için sabitlenmiş anahtarlardan biri değil", "key '{a}' is not one of the keys pinned for this application"),
                        ),
                        imza::Guven::Degisti => k(
                            "kart",
                            t!("Benlik Kartı", "Benlik Card"),
                            "uyari",
                            tf!(
                                "imza tutarlı ama anahtar değişmiş ('{a}') → sahibinden doğrulayın, sonra: sezgi anahtar sabitle",
                                "the signature is consistent but the key changed ('{a}') → verify it with the owner, then: sezgi key pin"
                            ),
                        ),
                        imza::Guven::Bilinmiyor => {
                            k("kart", t!("Benlik Kartı", "Benlik Card"), "tamam", tf!("imza geçerli (anahtar '{a}', sabitlenmemiş)", "signature valid (key '{a}', not pinned)"))
                        }
                    }),
                },
            },
        }
    }
}

fn rapor(args: &[String], proje: &Path, sonuc: Vec<Kontrol>) {
    let hata = sonuc.iter().filter(|x| x.durum == "hata").count();
    let uyari = sonuc.iter().filter(|x| x.durum == "uyari").count();
    if args.iter().any(|a| a == "--json") {
        let v = json!({
            "proje": proje.display().to_string().trim_start_matches(r"\\?\"),
            "hata": hata,
            "uyari": uyari,
            "kontroller": sonuc.iter().map(|x| json!({ "ad": x.ad, "durum": x.durum, "mesaj": x.mesaj })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    } else {
        tp!("Sezgi doktor: {}", "Sezgi doctor: {}", proje.display().to_string().trim_start_matches(r"\\?\"));
        for x in &sonuc {
            let i = match x.durum {
                "tamam" => "✓",
                "uyari" => "!",
                "hata" => "✗",
                _ => "·",
            };
            println!("  {i} {:<16} {}", x.baslik, x.mesaj);
        }
        println!();
        match (hata, uyari) {
            (0, 0) => tp!("Sorun yok.", "No problems."),
            (0, u) => tp!("Sorun yok, {u} uyarı.", "No problems, {u} {w}."; w = crate::dil::cogul(u, "warning", "warnings")),
            (h, u) => tp!("{h} sorun, {u} uyarı.", "{h} {p}, {u} {w}."; p = crate::dil::cogul(h, "problem", "problems"), w = crate::dil::cogul(u, "warning", "warnings")),
        }
    }
    if hata > 0 {
        std::process::exit(1);
    }
}
