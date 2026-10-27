//! `sezgi calistir [uygulama=.] [--cikti <klasör>] [--imzala [anahtar]] [--aralik <ms>]`: haritayı kurar, sonra kodu
//! izler ve yalnızca farkı öğrenir (vizyon 13).
//! - Değişiklik dosyaların parmak iziyle (değişim zamanı ve boyut) bulunur; .gitignore'a uyulur, node_modules okunmaz.
//!   İzlenen dosyalar: haritayı etkileyebilecek dosyalar ve haritanın okunan dosyaları (`ozet.okunan_dosyalar`).
//! - Yalnızca içeriği değişen dosyalar yeniden ayrıştırılır (NestJS, Express, FastAPI, Django, Flask, Rails, ASP.NET Core, Go); adaptör bütün model üzerinde yeniden çalışır
//!   (yetki, modül grafiği ve tablo bağları dosyalar arası olduğu için kısmi sonuç tutarsız olurdu).
//! - Kontrol sıklığı uyarlanır: değişiklik olunca `--aralik`, sessizken 4 katına kadar seyrekleşir.
//! - Harita atomik yazılır; çalışan `sezgi mcp` bir sonraki çağrıda yeni haritayı görür, hiçbir zaman yarım dosya okumaz.
//! - Uygulamanın hiçbir dosyasına yazılmaz.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{calisma_alani, guvenli, kesif, parmak, Onbellek, Parmak};

fn secenek(args: &[String], ad: &str) -> Option<String> {
    args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned()
}

/// Adaptörlerin okuduğu JavaScript / TypeScript uzantıları (NestJS, Express, Fastify, Next.js)
pub(crate) const JS_UZANTILARI: &[&str] = &["ts", "tsx", "js", "mjs", "cjs", "jsx"];
/// Adaptörlerin okuduğu Python uzantıları (FastAPI, Django, Flask)
pub(crate) const PY_UZANTILARI: &[&str] = &["py"];
/// Adaptörlerin okuduğu Ruby uzantıları (Rails; jbuilder yanıt şablonları)
pub(crate) const RB_UZANTILARI: &[&str] = &["rb", "jbuilder"];
/// Adaptörlerin okuduğu C# uzantıları (ASP.NET Core); proje dosyası (`.csproj`) çatı seçimini ve uygulamanın adını belirler
pub(crate) const CS_UZANTILARI: &[&str] = &["cs", "csproj"];
/// Adaptörlerin okuduğu Go uzantıları (net/http, Gin, Echo, Chi, Fiber, gorilla/mux)
pub(crate) const GO_UZANTILARI: &[&str] = &["go"];
/// Laravel adaptörünün okuduğu uzantı (PHP)
const PHP_UZANTILARI: &[&str] = &["php"];
/// Veritabanı şeması (tablo bağları)
const SEMA_UZANTILARI: &[&str] = &["prisma"];
/// Çatı seçimini, uygulamanın adını ya da bağımlılıklarını belirleyen dosyalar
const YAPILANDIRMA_DOSYALARI: &[&str] = &["package.json", "pyproject.toml", "requirements.txt", "requirements.in", "setup.cfg", "Pipfile", "Gemfile", "go.mod", "composer.json"];

/// Dosya haritayı etkiliyor mu. İzleme, bayatlık kapısı (`canli::bayat`), doktor ve paylaşım özeti bu tek listeyi
/// kullanır: bir adaptör yeni bir uzantı okumaya başlarsa yalnızca buraya eklenir
pub(crate) fn haritayi_etkiler(p: &Path) -> bool {
    // Go test dosyaları (`_test.go`) adaptörce okunmaz
    let go_testi = p.file_name().and_then(|a| a.to_str()).is_some_and(|a| a.ends_with("_test.go"));
    (!go_testi
        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                JS_UZANTILARI.contains(&e) || PY_UZANTILARI.contains(&e) || RB_UZANTILARI.contains(&e) || CS_UZANTILARI.contains(&e) || GO_UZANTILARI.contains(&e) || PHP_UZANTILARI.contains(&e) || SEMA_UZANTILARI.contains(&e)
            }))
        || p.file_name().and_then(|a| a.to_str()).is_some_and(|a| YAPILANDIRMA_DOSYALARI.contains(&a))
        // Spring Boot: Java ve Kotlin kaynakları, Maven / Gradle ve application.* yapılandırması
        || p.extension().and_then(|e| e.to_str()).is_some_and(|e| crate::spring::JAVA_UZANTILARI.contains(&e))
        || p.file_name().and_then(|a| a.to_str()).is_some_and(|a| crate::spring::YAPILANDIRMA_DOSYALARI.contains(&a))
        // Ktor: application.conf ve Gradle ayarları
        || p.file_name().and_then(|a| a.to_str()).is_some_and(|a| crate::ktor::YAPILANDIRMA_DOSYALARI.contains(&a))
}

/// Haritayı etkileyen dosyalar (`haritayi_etkiler`; .gitignore'a uyarak, bağlar, node_modules / dist ve sanal ortamlar
/// hariç)
pub(crate) fn ilgili_dosyalar(kokler: &[PathBuf]) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for kok in kokler {
        for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
            let p = g.path();
            if g.path_is_symlink() || !p.is_file() {
                continue;
            }
            let s = p.to_string_lossy();
            // .NET derleme çıktısı (bin / obj) her derlemede değişir: haritayı etkilemez
            let dotnet = p.strip_prefix(kok).unwrap_or(p).components().any(|c| matches!(c.as_os_str().to_str(), Some("bin" | "obj")));
            if s.contains("node_modules")
                || s.contains("\\dist\\")
                || s.contains("/dist/")
                || s.contains("site-packages")
                || s.contains("/vendor/")
                || s.contains("\\vendor\\")
                || dotnet
            {
                continue;
            }
            if haritayi_etkiler(p) {
                v.push(p.to_path_buf());
            }
        }
    }
    v
}

/// Haritanın okunan dosyaları (`ozet.okunan_dosyalar`): (yol, içerik özeti). Harita bu listeyi taşımıyorsa (eski
/// sürüm) None
pub(crate) fn okunan_dosyalar(kok: &Path, harita: &Path) -> Option<Vec<(PathBuf, String)>> {
    let v: serde_json::Value = serde_json::from_str(&guvenli::oku_harita(&harita.join("kapilar.json"))?).ok()?;
    let l = v["ozet"]["okunan_dosyalar"].as_array()?;
    Some(l.iter().filter_map(|d| Some((kesif::duzlestir(&kok.join(d["yol"].as_str()?)), d["ozet"].as_str()?.to_string()))).collect())
}

/// Haritadan sonra değişen dosyalar. Haritanın okuduğu dosyalar içerik özetiyle karşılaştırılır (değişen ya da silinen;
/// yalnızca dokunulan dosya değişmiş sayılmaz, değişim zamanı aynı kalsa da içerik değişikliği görülür). Haritanın
/// okumadığı ama haritayı etkileyebilecek dosyalar (yeni kaynak dosya) değişim zamanıyla bulunur. Bayatlık kapısı,
/// doktor ve ana ekran bunu kullanır.
/// İzlenen kökler: uygulama ve (haritada varsa) ön yüz klasörü
pub(crate) fn izlenen_kokler(kok: &Path, harita: &Path) -> Vec<PathBuf> {
    let mut v = vec![kok.to_path_buf()];
    let on_yuz = guvenli::oku_harita(&harita.join("kapilar.json"))
        .and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok())
        .and_then(|h| h["ozet"]["on_yuz"].as_str().map(|y| kesif::duzlestir(&kok.join(y))));
    if let Some(k) = on_yuz.filter(|k| !k.starts_with(kok)) {
        v.push(k);
    }
    v
}

pub(crate) fn degisen_dosyalar(kok: &Path, harita: &Path) -> Vec<PathBuf> {
    let zaman = harita.join("kapilar.json").metadata().and_then(|m| m.modified()).ok();
    let mut v = vec![];
    let mut bilinen = HashSet::new();
    for (p, o) in okunan_dosyalar(kok, harita).unwrap_or_default() {
        if guvenli::oku_bayt(&p).map(|b| kesif::icerik_ozeti(&b)).as_deref() != Some(o.as_str()) {
            v.push(p.clone());
        }
        bilinen.insert(p);
    }
    if let Some(z) = zaman {
        for p in ilgili_dosyalar(&izlenen_kokler(kok, harita)) {
            if !bilinen.contains(&kesif::duzlestir(&p)) && p.metadata().and_then(|m| m.modified()).is_ok_and(|d| d > z) {
                v.push(p);
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

fn parmaklar(kokler: &[PathBuf], okunan: &[PathBuf]) -> HashMap<PathBuf, Parmak> {
    ilgili_dosyalar(kokler).into_iter().chain(okunan.iter().cloned()).filter_map(|p| parmak(&p).map(|iz| (p, iz))).collect()
}

fn degisenler(eski: &HashMap<PathBuf, Parmak>, yeni: &HashMap<PathBuf, Parmak>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = yeni.iter().filter(|(p, iz)| eski.get(*p) != Some(iz)).map(|(p, _)| p.clone()).collect();
    v.extend(eski.keys().filter(|p| !yeni.contains_key(*p)).cloned());
    v.sort();
    v
}

fn goreli(kok: &Path, p: &Path) -> String {
    // Uygulamanın dışındaki dosya (ön yüz: ../frontend) da uygulamaya göre gösterilir
    crate::kesif::goreli_yol(kok, p).unwrap_or_else(|| p.display().to_string())
}

pub fn calistir(args: &[String]) {
    let aralik = secenek(args, "--aralik").and_then(|s| s.parse::<u64>().ok()).unwrap_or(500).clamp(50, 10_000);
    // Test ve betikler için: bu kadar güncellemeden sonra çık
    let en_fazla = secenek(args, "--en-fazla").and_then(|s| s.parse::<usize>().ok());
    let mut init_args = vec!["init".to_string()];
    let mut atla = false;
    for a in args.iter().skip(1) {
        if atla {
            atla = false;
            continue;
        }
        if a == "--aralik" || a == "--en-fazla" {
            atla = true;
            continue;
        }
        if a == "--deneme" {
            te!("✗ calistir --deneme ile çalışmaz: önce `sezgi init --deneme` ile bakın", "✗ watch does not work with --dry-run: look first with `sezgi init --dry-run`");
            std::process::exit(2);
        }
        init_args.push(a.clone());
    }
    let uygulama = init_args.get(1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| ".".into());
    let kok = std::fs::canonicalize(&uygulama).unwrap_or_else(|_| PathBuf::from(&uygulama));
    let mut kokler = vec![kok.clone()];
    kokler.extend(calisma_alani::bagimli_paketler(&kok).into_iter().map(|(_, k)| k));

    let cikti = secenek(&init_args, "--cikti").map(PathBuf::from).unwrap_or_else(|| kok.join(".sezgi"));
    let okunan = || okunan_dosyalar(&kok, &cikti).unwrap_or_default().into_iter().map(|(p, _)| p).collect::<Vec<_>>();
    let mut onbellek = Onbellek::default();
    let Some(ilk) = crate::init(&init_args, &mut onbellek, true) else { return };
    let mut izlenen = okunan();
    // Ön yüz uygulamanın dışındaysa (../frontend) o da izlenir
    for k in izlenen_kokler(&kok, &cikti).into_iter().skip(1) {
        if !kokler.contains(&k) {
            kokler.push(k);
        }
    }
    let mut izler = parmaklar(&kokler, &izlenen);
    let mut endpoint = ilk.endpoint;
    println!();
    tp!(
        "◆ İzleniyor: {} (değişiklik {} ms içinde fark edilir; durdurmak için Ctrl+C)",
        "◆ Watching: {} (changes are noticed within {} ms; Ctrl+C to stop)",
        kok.display().to_string().trim_start_matches(r"\\?\"),
        aralik
    );
    let mut bekle = aralik;
    let mut guncelleme = 0usize;
    loop {
        if en_fazla.is_some_and(|e| guncelleme >= e) {
            return;
        }
        std::thread::sleep(Duration::from_millis(bekle));
        let yeni = parmaklar(&kokler, &izlenen);
        let degisen = degisenler(&izler, &yeni);
        if degisen.is_empty() {
            bekle = (bekle * 2).min(aralik * 4);
            continue;
        }
        bekle = aralik;
        let Some(o) = crate::init(&init_args, &mut onbellek, false) else { return };
        let fark = o.endpoint as i64 - endpoint as i64;
        let fark = if fark == 0 { String::new() } else { format!(" ({fark:+})") };
        let ornek = degisen.iter().take(2).map(|p| goreli(&kok, p)).collect::<Vec<_>>().join(", ");
        let devami = if degisen.len() > 2 { format!(" +{}", degisen.len() - 2) } else { String::new() };
        tp!(
            "↻ {} · {} dosya değişti ({ornek}{devami}) → {} endpoint{fark} · {} / {} dosya yeniden ayrıştırıldı · {} ms",
            "↻ {} · {} {f} changed ({ornek}{devami}) → {} {e}{fark} · {} / {} {d} parsed again · {} ms",
            &crate::simdi()[11..19],
            degisen.len(),
            o.endpoint,
            o.ayristirilan,
            o.dosya,
            o.sure.as_millis();
            f = crate::dil::cogul(degisen.len(), "file", "files"),
            e = crate::dil::cogul(o.endpoint, "endpoint", "endpoints"),
            d = crate::dil::cogul(o.dosya, "file", "files")
        );
        endpoint = o.endpoint;
        izlenen = okunan();
        izler = parmaklar(&kokler, &izlenen);
        guncelleme += 1;
    }
}
