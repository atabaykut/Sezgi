//! `sezgi paylas [uygulama=.] [--imzala <anahtar>]`: ekibin yeniden kurulum yapmadan kullanacağı sıkıştırılmış harita
//! dosyası, `.sezgi/paylasim.bin` (vizyon 9.5). Git'e eklenebilir.
//!
//! İçerik: koddan çıkarılmış harita (kapilar.json), Benlik Kartı, (imzalıysa) açık anahtar belgesi ve her kaynak dosyanın
//! içerik özeti (SHA-256). Token, izin, kayıt ya da kullanıcıya ait bir şey yoktur.
//!
//! Depodan gelen dosya güvenilmezdir: sahte bir harita bir endpoint'i "herkese açık" gösterebilir. `sezgi init` bu
//! dosyayı ancak (1) sürüm ve adaptör aynıysa, (2) dosya bu uygulama için kullanıcının sabitlediği bir anahtarla
//! imzalıysa (`sezgi anahtar sabitle`; imza bağlamı kart imzasından ayrıdır) ve (3) her kaynak dosya paylaşıldığı
//! andakiyle birebir aynıysa kullanır. Biri tutmazsa nedenini söyler ve normal kurulumla devam eder.
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{calisma_alani, guvenli, imza, izle, kesif};

pub const DOSYA: &str = "paylasim.bin";
const BASLIK: &[u8] = b"SEZGI-PAYLASIM/1\n";
/// İçerik biçimi. 4: dosya özetleri SHA-256; haritanın okuduğu dosyaları ve haritayı etkileyebilecek bütün dosyaları kapsar
const BICIM: u64 = 4;
/// Açılmış içerik sınırı (sıkıştırma bombasına karşı)
const EN_BUYUK_ACIK: usize = 256 * 1024 * 1024;
const EN_BUYUK_DOSYA: u64 = 64 * 1024 * 1024;

fn surum() -> String {
    format!("sezgi {}", env!("CARGO_PKG_VERSION"))
}

/// Haritayı üreten adaptörün kimliği: init'in seçimiyle aynı (`crate::cati_sec`)
fn adaptor_kimligi(kok: &Path) -> String {
    let kurallar: crate::adaptor::Kurallar = toml::from_str(crate::NESTJS).expect("nestjs.toml");
    let pkg = guvenli::oku(&kok.join("package.json")).unwrap_or_default();
    crate::cati_sec(kok, &pkg, &kurallar).map(|s| crate::adaptor_kimligi(s, &kurallar)).unwrap_or_else(|| "?".into())
}

/// Haritanın okuduğu dosyalar (`ozet.okunan_dosyalar`, uygulama köküne göre)
fn okunan_yollari(kapilar: &Value) -> Vec<String> {
    kapilar["ozet"]["okunan_dosyalar"].as_array().into_iter().flatten().filter_map(|d| d["yol"].as_str().map(String::from)).collect()
}

/// Kaynak dosyaların içerik özetleri: haritanın okuduğu dosyalar ve haritayı etkileyebilecek bütün dosyalar (sonradan
/// eklenen kaynak dosya da görülsün); yollar deponun (yoksa uygulamanın) köküne göre, / ile
pub fn icerik_ozetleri(kok: &Path, okunan: &[String]) -> BTreeMap<String, String> {
    let taban = crate::git_koku(kok).unwrap_or_else(|| kok.to_path_buf());
    let mut kokler = vec![kok.to_path_buf()];
    kokler.extend(calisma_alani::bagimli_paketler(kok).into_iter().map(|(_, k)| k));
    let mut m = BTreeMap::new();
    let okunanlar = okunan.iter().map(|y| kesif::duzlestir(&kok.join(y)));
    for p in izle::ilgili_dosyalar(&kokler).into_iter().map(|p| kesif::duzlestir(&p)).chain(okunanlar) {
        let Ok(g) = p.strip_prefix(&taban) else { continue };
        if let Some(b) = guvenli::oku_bayt(&p) {
            m.insert(g.to_string_lossy().replace('\\', "/"), kesif::icerik_ozeti(&b));
        }
    }
    m
}

fn json_oku(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Paylaşım dosyasını açar ve doğrular; kullanılabilirse içeriği, değilse nedeni döner
pub fn dene(kok: &Path) -> Result<Value, String> {
    let yol = kok.join(".sezgi").join(DOSYA);
    let m = std::fs::symlink_metadata(&yol).map_err(|e| e.to_string())?;
    if !m.file_type().is_file() || m.len() > EN_BUYUK_DOSYA {
        return Err(t!("dosya düzenli bir dosya değil ya da çok büyük", "the file is not a regular file or is too large").into());
    }
    let ham = std::fs::read(&yol).map_err(|e| e.to_string())?;
    let govde = ham.strip_prefix(BASLIK).ok_or(t!("biçim tanınmadı", "format not recognized"))?;
    let acik =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(govde, EN_BUYUK_ACIK).map_err(|_| t!("dosya bozuk (açılamadı)", "file is corrupt (could not be unpacked)"))?;
    let v: Value = serde_json::from_slice(&acik).map_err(|_| t!("dosya bozuk (içerik okunamadı)", "file is corrupt (content could not be read)"))?;
    if v["tur"] != "sezgi-paylasim" {
        return Err(t!("biçim tanınmadı", "format not recognized").into());
    }
    // Biçim 1'in dosya özetleri JavaScript kaynaklarını kapsamıyordu: değişmiş kodla eski harita kullanılabilirdi
    match v["bicim"].as_u64() {
        Some(BICIM) => {}
        Some(b) if b < BICIM => {
            return Err(tf!(
                "eski biçimde ({b}) üretilmiş; özetleri bütün kaynak dosyaları kapsamıyor. Paylaşan yeniden üretin: sezgi paylas --imzala <anahtar>",
                "made in an old format ({b}); its digests do not cover every source file. The sharer should create it again: sezgi share --sign <key>"
            ))
        }
        _ => return Err(t!("biçim tanınmadı (daha yeni bir Sezgi ile üretilmiş olabilir)", "format not recognized (it may come from a newer Sezgi)").into()),
    }
    if v["uretici"].as_str() != Some(surum().as_str()) || v["adaptor"].as_str() != Some(adaptor_kimligi(kok).as_str()) {
        return Err(tf!("{} / {} ile üretilmiş, şimdiki {} / {}", "made with {} / {}, current is {} / {}", v["uretici"], v["adaptor"], surum(), adaptor_kimligi(kok)));
    }
    if v.get("signature").is_none() {
        return Err(t!(
            "imzasız: depodan gelen harita doğrulanamaz (ekip anahtarıyla imzalayın: sezgi paylas --imzala <anahtar>)",
            "unsigned: a map from the repository cannot be verified (sign it with the team key: sezgi share --sign <key>)"
        )
        .into());
    }
    let anahtarlar = &v["anahtarlar"];
    imza::dogrula_baglamla(&v, anahtarlar, imza::PAYLASIM_BAGLAMI).map_err(|e| tf!("imza: {e}", "signature: {e}"))?;
    match imza::guven_durumu(&v, anahtarlar) {
        imza::Guven::Sabit => {}
        _ => {
            return Err(t!(
                "imzalayan anahtar bu uygulama için sabitlenmemiş (sezgi anahtar sabitle <benlik.json> <benlik-keys.json>)",
                "the signing key is not pinned for this application (sezgi key pin <benlik.json> <benlik-keys.json>)"
            )
            .into())
        }
    }
    let simdi = icerik_ozetleri(kok, &okunan_yollari(&v["kapilar"]));
    let paylasilan: BTreeMap<String, String> = serde_json::from_value(v["dosyalar"].clone()).map_err(|_| t!("dosya listesi okunamadı", "the file list could not be read"))?;
    if simdi != paylasilan {
        let n = simdi.iter().filter(|(k, o)| paylasilan.get(*k) != Some(o)).count() + paylasilan.keys().filter(|k| !simdi.contains_key(*k)).count();
        return Err(tf!("{n} dosya paylaşımdan sonra değişmiş", "{n} {f} changed after sharing"; f = crate::dil::cogul(n, "file", "files")));
    }
    Ok(v)
}

/// .sezgi/.gitignore paylaşım dosyasını dışarıda bırakmasın; .gitattributes birleştirme denemesin (ikili)
fn git_ayarlari(sezgi: &Path) -> Result<(), String> {
    let gi = sezgi.join(".gitignore");
    let mut metin = std::fs::read_to_string(&gi).unwrap_or_else(|_| "# sezgi çıktısı: depoya eklenmez\n*\n".into());
    for satir in ["!.gitignore", "!.gitattributes", "!paylasim.bin"] {
        if !metin.lines().any(|l| l.trim() == satir) {
            if !metin.ends_with('\n') {
                metin.push('\n');
            }
            metin.push_str(satir);
            metin.push('\n');
        }
    }
    guvenli::atomik_yaz(&gi, metin.as_bytes())?;
    // binary: fark gösterilmez, birleştirilmez; iki kişi aynı anda paylaşırsa biri `sezgi paylas` ile yeniden üretir
    guvenli::atomik_yaz(
        &sezgi.join(".gitattributes"),
        "# sezgi paylas: harita dosyası birleştirilmez; çakışmada `sezgi paylas` ile yeniden üretin\npaylasim.bin binary\n".as_bytes(),
    )
}

pub fn calistir(args: &[String]) {
    let dur = |m: &str| -> ! {
        eprintln!("✗ {m}");
        std::process::exit(1)
    };
    let uygulama = args.get(1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| ".".into());
    let kok = std::fs::canonicalize(&uygulama).unwrap_or_else(|_| PathBuf::from(&uygulama));
    let anahtar = args.iter().position(|a| a == "--imzala").map(|i| args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "varsayilan".into()));
    // Harita koddan yeniden üretilir: paylaşılan harita ile özetlenen kod her zaman aynı anı gösterir
    let mut init_args = vec!["init".to_string(), kok.display().to_string(), "--paylasimsiz".to_string()];
    if let Some(a) = &anahtar {
        init_args.extend(["--imzala".to_string(), a.clone()]);
    }
    let sezgi = kok.join(".sezgi");
    if crate::init(&init_args, &mut crate::Onbellek::default(), false).is_none() {
        dur(t!("harita kurulamadı", "the map could not be built"));
    }
    let kapilar = json_oku(&sezgi.join("kapilar.json")).unwrap_or_else(|| dur(t!("kapilar.json okunamadı", "kapilar.json could not be read")));
    let kart = json_oku(&sezgi.join("benlik.json")).unwrap_or_else(|| dur(t!("benlik.json okunamadı", "benlik.json could not be read")));
    let mut v = json!({
        "tur": "sezgi-paylasim",
        "bicim": BICIM,
        "uretici": surum(),
        "adaptor": adaptor_kimligi(&kok),
        "app": kart["app"],
        "dosyalar": icerik_ozetleri(&kok, &okunan_yollari(&kapilar)),
        "kapilar": kapilar,
        "benlik": kart,
    });
    if let Some(a) = &anahtar {
        let belge = json_oku(&imza::acik_anahtar_dosyasi(a)).unwrap_or_else(|| dur(t!("açık anahtar belgesi okunamadı", "the public key document could not be read")));
        v["anahtarlar"] = belge;
        imza::imzala_baglamla(&mut v, a, imza::PAYLASIM_BAGLAMI).unwrap_or_else(|e| dur(&e));
    }
    let acik = serde_json::to_vec(&v).unwrap();
    let mut bin = BASLIK.to_vec();
    bin.extend(miniz_oxide::deflate::compress_to_vec_zlib(&acik, 9));
    if let Err(e) = guvenli::atomik_yaz(&sezgi.join(DOSYA), &bin).and_then(|_| git_ayarlari(&sezgi)) {
        dur(&e);
    }
    let n = v["kapilar"]["endpointler"].as_array().map(|a| a.len()).unwrap_or(0);
    tp!(
        "✓ Paylaşım dosyası: {} ({} KB, {} endpoint, {} kaynak dosya özeti)",
        "✓ Share file: {} ({} KB, {} {e}, {} source file {d})",
        guvenli::gorunen(&sezgi.join(DOSYA)),
        bin.len().div_ceil(1024),
        n,
        v["dosyalar"].as_object().map(|o| o.len()).unwrap_or(0);
        e = crate::dil::cogul(n, "endpoint", "endpoints"),
        d = crate::dil::cogul(v["dosyalar"].as_object().map(|o| o.len()).unwrap_or(0), "digest", "digests")
    );
    match &anahtar {
        Some(a) => tp!(
            "  İmza          '{a}' anahtarıyla. Ekip arkadaşları bu anahtarı bir kez sabitler; sonra sezgi init haritayı bu dosyadan kurar.",
            "  Signature     with key '{a}'. Teammates pin this key once; then sezgi init builds the map from this file."
        ),
        None => tp!(
            "  İmza          yok: ekip arkadaşlarının Sezgi'si bu dosyayı kullanmaz (depodan gelen harita doğrulanamaz). İmzalamak için: --imzala <anahtar>",
            "  Signature     none: your teammates' Sezgi will not use this file (a map from the repository cannot be verified). To sign: --sign <key>"
        ),
    }
    tp!(
        "  Depoya ekle   git add .sezgi/paylasim.bin .sezgi/.gitignore .sezgi/.gitattributes",
        "  Commit        git add .sezgi/paylasim.bin .sezgi/.gitignore .sezgi/.gitattributes"
    );
    tp!(
        "  Ne zaman      dosya her harita değişikliğinde değişir: her commit'te değil, sürümde ya da kilometre taşında paylaşın.",
        "  When          the file changes with every map change: share it at releases or milestones, not on every commit."
    );
}
