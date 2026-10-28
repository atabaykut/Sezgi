//! Santral (vizyon 14): birden fazla uygulama tek MCP sunucusunda; istemci ↔ kapı çapraz bağları (Kapı ③'ün "ne
//! bilinir" kısmı): hangi ekran dosyası hangi kapıyı çağırıyor. Birebir adres kesin, tahmin yorum; "aranmadı" ile
//! "arandı, bulunamadı" ayrı.
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

struct Ortam {
    kok: PathBuf,
    ev: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-santral-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        let o = Ortam { kok, ev };
        for (uyg, cikti) in [("nestjs-temel", "nest"), ("fastapi-temel", "fast")] {
            o.ok(&["init", ornek(uyg).to_str().unwrap(), "--cikti", o.kok.join(cikti).to_str().unwrap()]);
        }
        o
    }
    fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args).env("HOME", &self.ev).env("USERPROFILE", &self.ev).env("APPDATA", self.ev.join("ayar")).env("XDG_CONFIG_HOME", self.ev.join("ayar"));
        c
    }
    fn calistir(&self, args: &[&str]) -> Output {
        self.komut(args).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.calistir(args);
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    fn kaydet(&self) {
        self.ok(&["santral", "ekle", self.kok.join("nest").to_str().unwrap(), "--ad", "kutuphane", "--kaynak", ornek("nestjs-temel").to_str().unwrap()]);
        self.ok(&["santral", "ekle", self.kok.join("fast").to_str().unwrap(), "--ad", "kitapci", "--kaynak", ornek("fastapi-temel").to_str().unwrap()]);
        self.ok(&["santral", "ekle", "--istemci", ornek("istemci-temel").to_str().unwrap(), "--ad", "mobil"]);
    }
}

fn mcp(o: &Ortam, araclar: &[(&str, serde_json::Value)]) -> Vec<String> {
    let mut p = o.komut(&["mcp", "--santral"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = p.stdin.take().unwrap();
    let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
    let mut out = vec![];
    for (i, (ad, a)) in araclar.iter().enumerate() {
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": ad, "arguments": a } });
        writeln!(stdin, "{istek}").unwrap();
        stdin.flush().unwrap();
        let v: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
        out.push(v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string());
    }
    drop(stdin);
    let _ = p.wait();
    out
}

#[test]
fn birden_fazla_uygulama_tek_sunucuda() {
    let o = Ortam::yeni("mcp");
    o.kaydet();
    let liste = o.ok(&["santral"]);
    assert!(liste.contains("kutuphane") && liste.contains("kitapci") && liste.contains("mobil"), "{liste}");
    let c = mcp(
        &o,
        &[
            ("kimsin", serde_json::json!({})),
            ("ara", serde_json::json!({ "sorgu": "kitap" })),
            ("tani", serde_json::json!({ "kapi": "kitapci: POST /api/kitaplar/" })),
            ("tani", serde_json::json!({ "kapi": "POST /api/v1/kitaplar", "uygulama": "kutuphane" })),
            ("tani", serde_json::json!({ "kapi": "POST /api/kitaplar/" })),
        ],
    );
    assert!(c[0].contains("kutuphane") && c[0].contains("kitapci"), "kimsin bütün uygulamaları gösterir: {}", c[0]);
    assert!(c[1].contains("[kutuphane]") && c[1].contains("[kitapci]"), "ara bütün uygulamalarda: {}", c[1]);
    assert!(c[2].contains("ISBN"), "kitapci uygulamasının kartı: {}", c[2]);
    assert!(c[3].contains("baslik") && !c[3].contains("ISBN"), "kutuphane uygulamasının kartı: {}", c[3]);
    // Uygulama verilmezse kapı tek bir uygulamada bulunuyorsa o kullanılır
    assert!(c[4].contains("ISBN"), "{}", c[4]);
}

#[test]
fn capraz_baglar_kesin_ve_yorum() {
    let o = Ortam::yeni("bag");
    o.kaydet();
    let j: serde_json::Value = serde_json::from_str(&o.ok(&["santral", "baglar", "--json"])).unwrap();
    let baglar = j["baglar"].as_array().unwrap();
    let damga = |dosya: &str, kapi: &str| {
        baglar.iter().find(|b| b["dosya"].as_str().unwrap().ends_with(dosya) && b["kapi"] == kapi).unwrap_or_else(|| panic!("{dosya} -> {kapi} yok: {j}"))["damga"].clone()
    };
    assert_eq!(damga("KitapListesi.tsx", "kutuphane: GET /api/v1/kitaplar"), "kesin");
    assert_eq!(damga("KitapListesi.tsx", "kutuphane: POST /api/v1/kitaplar"), "kesin");
    assert_eq!(damga("KitapListesi.tsx", "kutuphane: DELETE /api/v1/kitaplar/:id"), "kesin");
    assert_eq!(damga("istemci.py", "kutuphane: GET /api/v1/kitaplar/:id/detay"), "kesin");
    // Önek istemcinin taban adresinde: tahmin
    let onekli = baglar.iter().find(|b| b["dosya"].as_str().unwrap().ends_with("KitapListesi.tsx") && b["metin"] == "/kitaplar").expect("önekli çağrı");
    assert_eq!(onekli["damga"], "yorum", "{onekli}");
    // Var olmayan adres bağlanmaz
    assert!(!baglar.iter().any(|b| b["metin"].as_str().unwrap_or("").contains("boyle-bir-yer-yok")));
    // Arama durumu dürüst: her kaynak/hedef çifti için "arandı" ya da "aranmadı"
    let durum = j["durum"].as_array().unwrap();
    assert!(durum.iter().any(|d| d["kaynak"] == "mobil" && d["hedef"] == "kutuphane" && d["durum"] == "arandi"), "{j}");
    // Uygulama kendi kodunda kendisine bağlanmaz
    assert!(!baglar.iter().any(|b| b["kaynak"] == "kutuphane" && b["kapi"].as_str().unwrap().starts_with("kutuphane:")));
    // MCP: bir kapıyı kimin çağırdığı
    let c = mcp(&o, &[("baglar", serde_json::json!({ "kapi": "kutuphane: DELETE /api/v1/kitaplar/:id" }))]);
    assert!(c[0].contains("KitapListesi.tsx"), "{}", c[0]);
}

#[test]
fn kaynagi_olmayan_aranmadi_olarak_bildirilir() {
    let o = Ortam::yeni("aranmadi");
    o.ok(&["santral", "ekle", o.kok.join("nest").to_str().unwrap(), "--ad", "kutuphane", "--kaynak", ornek("nestjs-temel").to_str().unwrap()]);
    // Kaynağı verilmemiş uygulama (harita başka bir yere kopyalanmış): kodu aranamaz
    let tasinmis = o.kok.join("fast-kopya");
    std::fs::create_dir_all(&tasinmis).unwrap();
    for f in ["kapilar.json", "benlik.json"] {
        std::fs::copy(o.kok.join("fast").join(f), tasinmis.join(f)).unwrap();
    }
    o.ok(&["santral", "ekle", tasinmis.to_str().unwrap(), "--ad", "kitapci"]);
    let j: serde_json::Value = serde_json::from_str(&o.ok(&["santral", "baglar", "--json"])).unwrap();
    assert!(j["durum"].as_array().unwrap().iter().any(|d| d["kaynak"] == "kitapci" && d["durum"] == "aranmadi"), "{j}");
    o.ok(&["santral", "cikar", "kitapci"]);
    assert!(!o.ok(&["santral"]).contains("kitapci"));
}

/// Santralde işlem tool'ları üye önekiyle verilir ve çağrı doğru üyeye gider
#[test]
fn santralde_islem_araclari_uye_onekli() {
    let o = Ortam::yeni("islem");
    o.kaydet();
    let m = o.ok(&["token", "olustur", "--ad", "santral", "--izin", "kitap.listele:oku", "--harita", o.kok.join("nest").to_str().unwrap()]);
    let token = m.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let mut p = o.komut(&["mcp", "--santral"]).env("SEZGI_TOKEN", &token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = p.stdin.take().unwrap();
    let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
    let mut iste = |v: serde_json::Value| -> serde_json::Value {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
        serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap()
    };
    let liste = iste(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }));
    let adlar: Vec<&str> = liste["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(adlar.contains(&"kutuphane__kitap_listele"), "{adlar:?}");
    assert!(!adlar.iter().any(|a| a.starts_with("kitapci__")), "token yalnızca kutuphane için: {adlar:?}");
    let c = iste(serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "kutuphane__kitap_listele", "arguments": {} } }));
    let metin = c["result"]["content"][0]["text"].as_str().unwrap();
    assert!(metin.contains("GÖZLEM MODU") && metin.contains("/api/v1/kitaplar"), "{metin}");
    let c = iste(serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "kitapci__kitap_listele", "arguments": {} } }));
    assert_eq!(c["result"]["isError"], true, "{c}");
    let _ = p.kill();
    let _ = p.wait();
}
