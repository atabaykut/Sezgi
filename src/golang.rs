//! Go kaynak kodu (tree-sitter-go): keşif, modül yolu (go.mod), paralel ayrıştırma ve çatı algılama. Çatı anlamı
//! (net/http, Gin, Echo, Chi, Fiber, gorilla/mux rotaları; ara katmanlar, gövde bağlama, GORM / SQL) bu modelin
//! üstünde `gohttp.rs` ile verilir.
//! - **Model:** dosya başına kaynak metni ve sözdizimi ağacı (önbellekte kopyalanması ucuz). Adaptör ağacın üstünde
//!   çalışır: rotaları kuran kodu sırasıyla yorumlar, işleyicileri ve ara katmanları tanımlarından okur.
//! - **Paketler:** bir dosyanın içe aktarma yolu, en yakın üst klasördeki `go.mod`'un modül yolu + modül köküne göre
//!   klasörüdür (çok modüllü depolar dahil). Test dosyaları (`_test.go`), `vendor/`, `testdata/` ve `_` / `.` ile
//!   başlayan klasörler Go aracının da yaptığı gibi atlanır.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tree_sitter::Parser;

use crate::guvenli;

/// Ayrıştırılmış Go dosyası: kaynak ve ağaç (önbellekte paylaşılır)
#[derive(Clone)]
pub(crate) struct Dosya {
    pub yol: PathBuf,
    pub kaynak: Arc<str>,
    pub agac: tree_sitter::Tree,
    pub satir_sayisi: usize,
    pub hatali: bool,
}

/// Bir `go.mod`: bulunduğu klasör ve modül yolu
#[derive(Clone, Debug)]
pub(crate) struct Modul {
    pub klasor: PathBuf,
    pub yol: String,
}

/// `go.mod` metninden modül yolu
fn modul_yolu(metin: &str) -> Option<String> {
    metin.lines().map(str::trim).find_map(|l| l.strip_prefix("module ")).map(|m| m.trim().trim_matches('"').to_string()).filter(|m| !m.is_empty())
}

/// Go uygulaması mı: kökte `go.mod` ya da `go.work`; modülsüz (GOPATH) projede kökte `.go` dosyası ya da `Gopkg.toml`
pub fn go_mi(kok: &Path) -> bool {
    kok.join("go.mod").is_file()
        || kok.join("go.work").is_file()
        || kok.join("Gopkg.toml").is_file()
        || std::fs::read_dir(kok).is_ok_and(|o| o.flatten().any(|g| g.path().extension().is_some_and(|e| e == "go") && g.path().is_file()))
}

/// Modülsüz (GOPATH) projenin içe aktarma öneki: içe aktarılan bir yolun sonu projedeki bir klasörün göreli yoluysa
/// (`github.com/x/uyg/app/handler` ve `app/handler`) baştaki kısım. En çok eşleşen önek seçilir
fn modulsuz_onek(kok: &Path, dosyalar: &[Dosya]) -> Option<String> {
    let klasorler: std::collections::BTreeSet<String> = dosyalar
        .iter()
        .filter_map(|d| d.yol.parent()?.strip_prefix(kok).ok().map(|g| g.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")))
        .filter(|g| !g.is_empty())
        .collect();
    let mut sayim: HashMap<String, usize> = HashMap::new();
    for d in dosyalar {
        for satir in d.kaynak.lines().map(str::trim) {
            let yol = satir.rsplit(' ').next().unwrap_or(satir).trim_matches('"');
            if !satir.ends_with('"') || !yol.contains('/') {
                continue;
            }
            for k in &klasorler {
                if let Some(on) = yol.strip_suffix(k.as_str()).and_then(|o| o.strip_suffix('/')) {
                    *sayim.entry(on.to_string()).or_default() += 1;
                }
            }
        }
    }
    sayim.into_iter().max_by_key(|(o, n)| (*n, std::cmp::Reverse(o.clone()))).map(|(o, _)| o)
}

/// Gereksinimlerdeki (go.mod `require`) HTTP çatısı; yoksa standart kitaplık (net/http)
pub fn cati_adi(kok: &Path) -> &'static str {
    let m = guvenli::oku(&kok.join("go.mod")).unwrap_or_default();
    let var = |p: &str| m.lines().any(|l| l.split_whitespace().any(|s| s == p || s.starts_with(&format!("{p}/v"))));
    if var("github.com/gin-gonic/gin") {
        "Gin"
    } else if var("github.com/labstack/echo") {
        "Echo"
    } else if var("github.com/gofiber/fiber") {
        "Fiber"
    } else if var("github.com/go-chi/chi") {
        "Chi"
    } else if var("github.com/gorilla/mux") {
        "gorilla/mux"
    } else {
        "net/http"
    }
}

/// Uygulamanın adı (modül yolunun son parçası, `/vN` sürüm eki hariç) ve sürümü (Go modüllerinde yok)
pub fn paket(kok: &Path) -> Value {
    let ad = guvenli::oku(&kok.join("go.mod")).and_then(|m| modul_yolu(&m)).map(|m| {
        let mut p: Vec<&str> = m.split('/').collect();
        if p.len() > 1 && p.last().is_some_and(|s| s.len() > 1 && s.starts_with('v') && s[1..].chars().all(|c| c.is_ascii_digit())) {
            p.pop();
        }
        p.last().unwrap_or(&"").to_string()
    });
    json!({ "name": ad, "version": null })
}

/// Go aracının atladığı ya da uygulamanın API'si olmayan klasörler
fn atlanir(ad: &str) -> bool {
    ad.starts_with('.') || ad.starts_with('_') || matches!(ad, "vendor" | "testdata" | "node_modules")
}

fn ayristir(yol: &Path, b: Vec<u8>) -> Option<Dosya> {
    let kaynak = String::from_utf8(b).ok()?;
    let mut p = Parser::new();
    p.set_language(&tree_sitter_go::LANGUAGE.into()).ok()?;
    let agac = p.parse(&kaynak, None)?;
    let hatali = agac.root_node().has_error();
    Some(Dosya { yol: yol.to_path_buf(), satir_sayisi: kaynak.lines().count(), kaynak: Arc::from(kaynak), agac, hatali })
}

/// Keşif, paralel ayrıştırma ve Go HTTP adaptörü
pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
    let mut yollar = vec![];
    let mut moduller: Vec<Modul> = vec![];
    let mut okunan_ek = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    let mut gezgin = ignore::WalkBuilder::new(kok);
    gezgin.hidden(true).git_ignore(true).filter_entry(|g| g.depth() == 0 || !g.file_type().is_some_and(|t| t.is_dir()) || !atlanir(&g.file_name().to_string_lossy()));
    for g in gezgin.build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            if g.path_is_symlink() {
                atlanan += 1;
            }
            continue;
        }
        let ad = p.file_name().and_then(|a| a.to_str()).unwrap_or("");
        if ad == "go.mod" {
            if let Some(m) = guvenli::oku(p) {
                okunan_ek.push((p.to_path_buf(), crate::kesif::icerik_ozeti(m.as_bytes())));
                if let Some(y) = modul_yolu(&m) {
                    moduller.push(Modul { klasor: p.parent().unwrap_or(kok).to_path_buf(), yol: y });
                }
            }
            continue;
        }
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| crate::izle::GO_UZANTILARI.contains(&e)) && !ad.ends_with("_test.go") {
            if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                buyuk.push(guvenli::goreli(kok, p));
                continue;
            }
            yollar.push(p.to_path_buf());
        } else {
            atlanan += 1;
        }
    }
    // En derin modül önce: dosya en yakın go.mod'a aittir
    moduller.sort_by_key(|m| std::cmp::Reverse(m.klasor.components().count()));
    let t_kesif = t0.elapsed();
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, ayristir);
    okunan.extend(okunan_ek);
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    if moduller.is_empty() {
        if let Some(on) = modulsuz_onek(kok, &dosyalar) {
            moduller.push(Modul { klasor: kok.to_path_buf(), yol: on });
        }
    }
    let t_ayristirma = t0.elapsed();
    let sonuc = crate::gohttp::kapilar(kok, &dosyalar, &moduller);
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar: sonuc.kapilar,
        bilgi: sonuc.bilgi,
        adaptor: format!("go@{}", crate::gohttp::SURUM),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| d.hatali).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        // Tablo tanımlayan modellerin dosyaları (GORM): rapor tablo bağlarını bunlara göre sayar
        veri_dosyalari: sonuc.veri_dosyalari,
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

/// Dosyanın içe aktarma yolu: en yakın go.mod'un modül yolu + modül köküne göre klasörü (`moduller` derinden sığa
/// sıralı). Hiçbir modüle ait değilse klasörün köke göre yolu
pub(crate) fn ice_aktarma_yolu(kok: &Path, moduller: &[Modul], dosya: &Path) -> String {
    let klasor = dosya.parent().unwrap_or(kok);
    let parcalar =
        |taban: &Path| -> Vec<String> { klasor.strip_prefix(taban).map(|g| g.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect()).unwrap_or_default() };
    for m in moduller {
        if klasor.starts_with(&m.klasor) {
            let p = parcalar(&m.klasor);
            return if p.is_empty() { m.yol.clone() } else { format!("{}/{}", m.yol, p.join("/")) };
        }
    }
    parcalar(kok).join("/")
}
