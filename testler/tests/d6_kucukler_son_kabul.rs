//! D6 küçük eksikler (son kabul testi): bağlantı komutu kurulum yerine bağlı değil ve `token olustur` ile aynı;
//! çıkış kodları tutarlı (2 kullanım, 1 çalışma hatası); `init` imza ipucu ve olmayan klasör; terminalsiz oturum;
//! rapor başlığı ve onay nedeni; kanıt paketi uyarıları gruplu; kaynaksız `kimsin` tek paragraf; bayat işlem sayısı
//! listeyle tutarlı; SQLite yabancı anahtarı ve kısıt ayrıntıları; işleme uymayan izin deseni; santral dosyaları
//! sahibine özel.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn metin(c: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
}

/// Projede verilen ikiliyle (ör. node_modules altına kopyalanmış) komut
fn ikiliyle(p: &Proje, exe: &Path, args: &[&str]) -> Output {
    Command::new(exe)
        .args(args)
        .current_dir(&p.klasor)
        .env("HOME", &p.ev)
        .env("USERPROFILE", &p.ev)
        .env("APPDATA", p.ev.join("ayar"))
        .env("XDG_CONFIG_HOME", p.ev.join("ayar"))
        .env_remove("SEZGI_TOKEN")
        .env("SEZGI_ANAHTARLIK", "kapali")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// İkiliyi projenin `node_modules` klasörüne, npm platform paketinin yerine kopyalar
fn node_modules_ikilisi(p: &Proje) -> PathBuf {
    let k = p.klasor.join("node_modules").join("@sezgi").join("cli-linux-x64").join("bin");
    std::fs::create_dir_all(&k).unwrap();
    let hedef = k.join(if cfg!(windows) { "sezgi.exe" } else { "sezgi" });
    std::fs::copy(env!("CARGO_BIN_EXE_sezgi"), &hedef).unwrap();
    hedef
}

/// Çıktıdaki "claude mcp add" satırı (token varsa `-e SEZGI_TOKEN=…` çıkarılmış)
fn claude_satiri(m: &str) -> String {
    let s = m.lines().find(|l| l.contains("claude mcp add")).unwrap_or_else(|| panic!("claude mcp add satırı yok: {m}"));
    let s = &s[s.find("claude mcp add").unwrap()..];
    s.split_whitespace().filter(|w| !w.starts_with("SEZGI_TOKEN=") && *w != "-e").collect::<Vec<_>>().join(" ")
}

#[test]
fn bagla_node_modules_icinde_kalici_npx_komutu_onerir() {
    let ev = Ev::yeni("d6k-npx");
    let p = ev.proje("nestjs-temel", "a");
    let exe = node_modules_ikilisi(&p);
    let surum = env!("CARGO_PKG_VERSION");
    let elle = metin(&ikiliyle(&p, &exe, &["bagla", "--elle"]));
    let satir = claude_satiri(&elle);
    assert!(satir.contains(&format!("npx -y sezgi@{surum} mcp")), "node_modules içindeki ikili yerine sürümü sabit npx: {elle}");
    assert!(!elle.contains("node_modules"), "kurulum yerine bağlı mutlak yol yazılmaz: {elle}");
    // token olustur aynı kaynaktan aynı komutu önerir
    let c = ikiliyle(&p, &exe, &["token", "olustur", "--ad", "ajan", "--izin", "kitap.*:oku"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert_eq!(claude_satiri(&metin(&c)), satir, "bagla --elle ve token olustur aynı komutu önerir");
    // bagla (dosyaya yazan) da aynı komutu yazar
    let c = ikiliyle(&p, &exe, &["bagla", "--arac", "claude-code"]);
    assert!(c.status.success(), "{}", metin(&c));
    let ayar: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".mcp.json")).unwrap()).unwrap();
    let g = &ayar["mcpServers"]["sezgi"];
    assert_eq!(g["command"], "npx", "{ayar}");
    assert_eq!(g["args"][1], format!("sezgi@{surum}"), "{ayar}");
    assert!(!ayar.to_string().contains("node_modules"), "{ayar}");
}

#[test]
fn token_olustur_ve_bagla_elle_ayni_komutu_onerir() {
    let ev = Ev::yeni("d6k-tutarli");
    let p = ev.proje("nestjs-temel", "a");
    let elle = p.ok(&["bagla", "--elle"]);
    let tok = p.ok(&["token", "olustur", "--ad", "ajan", "--izin", "kitap.*:oku"]);
    assert_eq!(claude_satiri(&tok), claude_satiri(&elle), "{tok}\n---\n{elle}");
}

/// (çıkış kodu, çıktı)
fn kod(p: &Proje, args: &[&str]) -> (i32, String) {
    let c = p.sezgi(args, None);
    (c.status.code().unwrap_or(-1), metin(&c))
}

#[test]
fn eksik_ya_da_yanlis_arguman_her_yerde_2_calisma_hatasi_1() {
    let ev = Ev::yeni("d6k-kod");
    let p = ev.proje("nestjs-temel", "a");
    for args in [
        &["token", "olustur"][..],
        &["token", "olustur", "--ad", "x"],
        &["token", "olustur", "--ad", "x", "--izin", "kitap.*:oku", "--onay", "bazen"],
        &["token", "olustur", "--ad", "x", "--izin", "kitap.*:oku", "--sure", "999"],
        &["token", "olustur", "--ad", "x", "--izin", "bozuk"],
        &["token", "iptal"],
        &["onay", "ver"],
        &["onay", "goster"],
        &["plan", "goster"],
        &["plan", "onayla"],
        &["santral", "cikar"],
        &["santral", "ekle"],
        &["unut"],
        &["canli", "kur"],
        &["oturum", "ac", "--oidc", "http://127.0.0.1:9"],
        &["defter", "--saklama", "3"],
        &["dogrula"],
        &["karsilastir"],
        &["gozlem", "al"],
    ] {
        let (k, m) = kod(&p, args);
        assert_eq!(k, 2, "{args:?} kullanım hatası 2 ile çıkar: {m}");
        assert!(m.contains("Kullanım") || m.contains("sezgi "), "{args:?}: nasıl kullanılacağı söylenir: {m}");
    }
    for args in [&["onay", "ver", "yok"][..], &["token", "iptal", "yok"], &["santral", "cikar", "yok"], &["karsilastir", "/yok/a", "/yok/b"], &["init", "/yok/klasor"]] {
        let (k, m) = kod(&p, args);
        assert_eq!(k, 1, "{args:?} çalışma hatası 1 ile çıkar: {m}");
    }
}

#[test]
fn yapilmis_onayi_yeniden_vermek_hata_ile_cikar_ve_neden_kurali_soyler() {
    let ev = Ev::yeni("d6k-onay");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(kitap_olustur());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    // Onay nedeni denetlenemeyen kuralı adıyla söyler
    assert!(s.contains("ValidateIf") && s.contains("cevirmen"), "hangi kuralın denetlenemediği yazılır: {s}");
    let id = onay_kimligi(&s);
    let oz = p.onay_ozeti(&id);
    p.ok(&["onay", "ver", &id, "--ozet", &oz]);
    let (k, m) = kod(&p, &["onay", "ver", &id, "--ozet", &oz]);
    assert_eq!(k, 1, "yapılmış onayı yeniden vermek hata: {m}");
    assert!(m.contains("bekleyen onay yok"), "{m}");
    assert_eq!(u.sayi(), 1);
}

#[test]
fn init_imza_ipucu_once_anahtar_olusturmayi_soyler() {
    let ev = Ev::yeni("d6k-ipucu");
    let p = ev.proje("nestjs-temel", "a");
    let m = p.ok(&["init", "."]);
    let a = m.find("sezgi anahtar olustur").unwrap_or_else(|| panic!("anahtar yokken önce anahtar olustur önerilir: {m}"));
    let i = m.find("--imzala").unwrap_or_else(|| panic!("{m}"));
    assert!(a < i, "önce anahtar, sonra --imzala: {m}");
    p.ok(&["anahtar", "olustur"]);
    let m = p.ok(&["init", "."]);
    assert!(m.contains("sezgi init --imzala") && !m.contains("anahtar olustur"), "anahtar varken doğrudan --imzala: {m}");
}

#[test]
fn init_olmayan_klasorde_klasor_yok_der() {
    let ev = Ev::yeni("d6k-yok");
    let p = ev.proje("nestjs-temel", "a");
    let (k, m) = kod(&p, &["init", "yokklasor"]);
    assert_eq!(k, 1, "{m}");
    assert!(m.contains("klasör yok: yokklasor"), "{m}");
    assert!(!m.contains("okuyabildiği bir uygulama"), "{m}");
}

#[test]
fn terminalsiz_oturum_ne_beklendigini_soyler() {
    let ev = Ev::yeni("d6k-oturum");
    let p = ev.proje("nestjs-temel", "a");
    let (k, m) = kod(&p, &["oturum", "ac", "--rol", "ADMIN"]);
    assert_eq!(k, 2, "{m}");
    assert!(m.contains("stdin") && m.contains("tek satır") && m.contains("--oidc"), "oturum değerinin nereden beklendiği yazılır: {m}");
    let y = p.ok(&["yardim", "oturum"]);
    assert!(y.contains("stdin"), "yardım da söyler: {y}");
}

#[test]
fn rapor_basliginda_git_yoksa_anlamli_metin() {
    let ev = Ev::yeni("d6k-rapor");
    let p = ev.proje("nestjs-temel", "a");
    let m = p.ok(&["rapor"]);
    let satir = m.lines().find(|l| l.contains("Sürüm")).unwrap_or_else(|| panic!("{m}"));
    assert!(!satir.contains("unknown"), "{satir}");
    assert!(satir.contains("git yok"), "{satir}");
}

#[test]
fn bozuk_kanit_paketi_uyarilari_gruplanir() {
    let ev = Ev::yeni("d6k-paket");
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    let yol = p.klasor.join(".sezgi").join("benlik-evidence.json");
    let paket = std::fs::read_to_string(&yol).unwrap();
    let toplam = paket.matches("\"certain\"").count();
    assert!(toplam > 10, "{toplam}");
    std::fs::write(&yol, paket.replace("\"certain\"", "\"inferred\"")).unwrap();
    let (k, m) = kod(&p, &["dogrula", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    assert_eq!(k, 1, "{m}");
    let satirlar: Vec<&str> = m.lines().filter(|l| l.contains("kayıt özeti uyuşmuyor")).collect();
    assert_eq!(satirlar.len(), 1, "tek özet satırı: {m}");
    assert!(satirlar[0].contains("+"), "ilk birkaç örnek ve kalanın sayısı: {}", satirlar[0]);
    assert!(m.lines().count() < 15, "çıktı kısa: {m}");
    // Özeti tutmayan kayıtlar "sunulmadı" diye ikinci kez sayılmaz
    assert!(!m.contains("sunulmadı"), "{m}");
}

#[test]
fn bayat_islem_sayisi_listeyle_tutarli() {
    let ev = Ev::yeni("d6k-bayat");
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    let d = p.klasor.join("src").join("kitap").join("kitap.controller.ts");
    let mut s = std::fs::read_to_string(&d).unwrap();
    s.push_str("\n// değişti\n");
    std::fs::write(&d, s).unwrap();
    let (_, m) = kod(&p, &["dogrula", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    let satir = m.lines().find(|l| l.contains("işlem bayat")).unwrap_or_else(|| panic!("{m}"));
    let n: usize = satir.trim_start_matches(['!', ' ']).split_whitespace().next().unwrap().parse().unwrap();
    let liste = &satir[satir.find('(').unwrap() + 1..satir.find(')').unwrap()];
    let adlar: Vec<&str> = liste.split(", ").collect();
    let mut tekil = adlar.clone();
    tekil.sort();
    tekil.dedup();
    assert_eq!(tekil.len(), adlar.len(), "aynı ad iki kez yazılmaz: {satir}");
    let kalan: usize = adlar.last().and_then(|a| a.strip_prefix("+")).and_then(|a| a.split_whitespace().next()).and_then(|a| a.parse().ok()).unwrap_or(0);
    let gosterilen = adlar.len() - usize::from(kalan > 0);
    assert_eq!(gosterilen + kalan, n, "sayı ile listelenenler tutarlı: {satir}");
    assert!(n > 5, "bu örnekte kitap denetleyicisinin bütün işlemleri bayat: {satir}");
}

fn mcp_metinleri(p: &Proje, harita: &str, araclar: &[(&str, serde_json::Value)]) -> Vec<String> {
    use std::io::{BufRead, BufReader, Write};
    let mut c = p.komut(&["mcp", harita]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut girdi = c.stdin.take().unwrap();
    let mut satirlar = BufReader::new(c.stdout.take().unwrap()).lines();
    let mut out = vec![];
    for (i, (ad, a)) in araclar.iter().enumerate() {
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": ad, "arguments": a } });
        writeln!(girdi, "{istek}").unwrap();
        girdi.flush().unwrap();
        let v: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
        out.push(v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string());
    }
    drop(girdi);
    let _ = c.kill();
    let _ = c.wait();
    out
}

/// SQLite örneği (musteri ← siparis, ON DELETE CASCADE, CHECK, NOT NULL, UNIQUE) kaynaksız olarak keşfedilir
fn dukkan(ev: &Ev) -> Proje {
    let klasor = ev.kok.join("dukkan");
    std::fs::create_dir_all(&klasor).unwrap();
    std::fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/d6_kucukler_dukkan/dukkan.db"), klasor.join("dukkan.db")).unwrap();
    let p = Proje { klasor, ev: ev.kok.join("ev") };
    p.ok(&["kesfet", "dukkan.db", "--cikti", "db-cikti"]);
    p
}

#[test]
fn kaynaksiz_kimsin_paragrafi_bir_kez_yazar() {
    let ev = Ev::yeni("d6k-kimsin");
    let p = dukkan(&ev);
    let r = mcp_metinleri(&p, "db-cikti", &[("kimsin", serde_json::json!({}))]);
    assert_eq!(r[0].matches("Kaynaksız uygulama (").count(), 1, "{}", r[0]);
}

#[test]
fn sqlite_yabanci_anahtari_gozlem_ve_kisitlar_gorunur() {
    let ev = Ev::yeni("d6k-sqlite");
    let p = dukkan(&ev);
    let r = mcp_metinleri(&p, "db-cikti", &[("kanit", serde_json::json!({ "varlik": "db:siparis" })), ("kanit", serde_json::json!({ "varlik": "field:siparis.tutar" }))]);
    let k = &r[0];
    let fk = k.lines().find(|l| l.starts_with("REFERENCES db:musteri")).unwrap_or_else(|| panic!("{k}"));
    // Şemada açıkça tanımlı FK: dosyadan okunan şemanın tavanı (gözlem); yorum değil
    assert!(fk.contains("[Gözlem]") && !fk.contains("[Yorum]"), "{fk}");
    assert!(fk.contains("musteri_id") && fk.contains("ON DELETE CASCADE") && fk.contains("ON UPDATE RESTRICT"), "{fk}");
    let kisit = k.lines().find(|l| l.starts_with("KISITLAR")).unwrap_or_else(|| panic!("{k}"));
    assert!(kisit.contains("tutar > 0") && kisit.contains("NOT NULL") && kisit.contains("musteri_id"), "{kisit}");
    let var = k.lines().find(|l| l.starts_with("var")).unwrap_or_else(|| panic!("{k}"));
    assert!(var.contains("tutar") && var.contains("durum"), "sütunlar görünür: {var}");
    assert!(r[1].contains("NOT NULL") && r[1].contains("CHECK"), "{}", r[1]);
    // Kanıt deposunda da ayrıntı var; graf kenarı observed
    let graf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join("db-cikti/graf.json")).unwrap()).unwrap();
    let kenar = graf["kenarlar"].as_array().unwrap().iter().find(|e| e["kaynak"] == "db:siparis" && e["iliski"] == "REFERENCES").unwrap();
    assert_eq!(kenar["guven"], "observed", "{kenar}");
    let depo = std::fs::read_to_string(p.klasor.join("db-cikti/kanit.json")).unwrap();
    assert!(depo.contains("CASCADE") && depo.contains("RESTRICT"), "kısıt ayrıntısı kanıtta");
    // Kaskadın olası etkisi hiçbir zaman kesin değil
    let etki = graf["kenarlar"].as_array().unwrap().iter().find(|e| e["iliski"] == "MAY_AFFECT").unwrap();
    assert_eq!(etki["guven"], "inferred");
}

#[test]
fn hicbir_isleme_uymayan_izin_uyarir_kesin_ile_hata() {
    let ev = Ev::yeni("d6k-izin");
    let p = ev.proje("nestjs-temel", "a");
    let (k, m) = kod(&p, &["token", "olustur", "--ad", "x", "--izin", "kitp.*:oku"]);
    assert_eq!(k, 0, "{m}");
    assert!(m.contains("hiçbir işleme uymuyor") && m.contains("kitap.*"), "en yakın desen önerilir: {m}");
    let (k, m) = kod(&p, &["token", "olustur", "--ad", "y", "--izin", "yokislem:oku", "--kesin"]);
    assert_eq!(k, 2, "{m}");
    assert!(m.contains("hiçbir işleme uymuyor") && !m.contains("szg_"), "token üretilmez: {m}");
    // Uyan desende uyarı yok
    let (_, m) = kod(&p, &["token", "olustur", "--ad", "z", "--izin", "kitap.*:oku", "--kesin"]);
    assert!(!m.contains("hiçbir işleme uymuyor") && m.contains("szg_"), "{m}");
}

#[test]
fn mod_yazma_mesaji_surum_numarasi_icermez() {
    let ev = Ev::yeni("d6k-mod");
    let p = ev.proje("nestjs-temel", "a");
    let (k, m) = kod(&p, &["mod", "yazma"]);
    assert_eq!(k, 1, "{m}");
    assert!(!m.contains("0.2"), "{m}");
}

#[cfg(unix)]
#[test]
fn santral_dosyalari_sahibine_ozel() {
    use std::os::unix::fs::PermissionsExt;
    let ev = Ev::yeni("d6k-santral");
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["santral", "ekle", ".sezgi", "--ad", "kitaplik"]);
    p.ok(&["santral", "baglar"]);
    for ad in ["santral.json", "santral-baglar.json"] {
        let yol = p.ev.join("ayar").join("sezgi").join(ad);
        let kip = std::fs::metadata(&yol).unwrap_or_else(|e| panic!("{}: {e}", yol.display())).permissions().mode() & 0o777;
        assert_eq!(kip, 0o600, "{ad} yalnızca sahibine açık");
    }
}
