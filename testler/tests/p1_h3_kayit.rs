//! P1 H3: canlı mod durum dosyalarının süreçler arası kilidi (B11), O(1) kayıt ekleme, santralde token'lı kullanım ve
//! tekrar çalıştırılabilir `canli kur`.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

/// Durum klasörü (uygulamalar/<proje kimliği>): kayit.jsonl'ın bulunduğu tek klasör
fn durum_klasoru(ev: &Ev) -> std::path::PathBuf {
    let k = ev.ayar().join("sezgi").join("uygulamalar");
    let mut v: Vec<_> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.join("kayit.jsonl").exists()).collect();
    assert_eq!(v.len(), 1, "{v:?}");
    v.pop().unwrap()
}

fn satir_sayisi(p: &Proje) -> usize {
    p.ok(&["kayit", "--json"]).lines().filter(|l| !l.trim().is_empty()).count()
}

/// Aynı anda çalışan sezgi süreçleri aynı kayda ve aynı ayar / token dosyalarına yazar: zincir bozulmaz, satır ve
/// token kaybolmaz
#[test]
fn eszamanli_surecler_zinciri_ve_tokenleri_bozmaz() {
    let ev = Ev::yeni("h3-eszamanli");
    let p = ev.proje("nestjs-temel", "a");
    let u = Uygulama::yeni();
    p.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
    let once = satir_sayisi(&p);
    let (is_parcacigi, tur) = (8, 6);
    std::thread::scope(|s| {
        for i in 0..is_parcacigi {
            let p = &p;
            s.spawn(move || {
                for j in 0..tur {
                    let m = if (i + j) % 2 == 0 { "okuma" } else { "gozlem" };
                    p.ok(&["mod", m]);
                }
                p.ok(&["token", "olustur", "--ad", &format!("t{i}"), "--izin", "kitap.*:oku"]);
            });
        }
    });
    let d = p.ok(&["kayit", "--dogrula"]);
    assert!(d.contains("sağlam"), "{d}");
    // Her mod değişikliği ve her token üretimi bir satır
    assert_eq!(satir_sayisi(&p), once + is_parcacigi * tur + is_parcacigi, "satır kayboldu ya da fazladan satır var");
    let l = p.ok(&["token", "listele"]);
    for i in 0..is_parcacigi {
        assert!(l.contains(&format!("t{i}")), "token t{i} kayboldu (eşzamanlı yazımda üzerine yazıldı): {l}");
    }
    let m = p.ok(&["mod"]);
    assert!(m.contains("Mod: okuma") || m.contains("Mod: gozlem"), "{m}");
    assert!(!durum_klasoru(&ev).join(".kilit").exists(), "kilit dosyası bırakılmalı");
}

/// Sahibi çökmüş (bayat) kilit kaydı sonsuza kadar kilitlemez; taze kilit beklenir
#[test]
fn bayat_kilit_temizlenir() {
    let ev = Ev::yeni("h3-bayat-kilit");
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["mod", "okuma"]);
    let kilit = durum_klasoru(&ev).join(".kilit");
    std::fs::write(&kilit, "999999").unwrap();
    // Değişiklik zamanı geriye alınır: sahibi çoktan ölmüş bir süreç
    let eski = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&kilit).unwrap().set_modified(eski).unwrap();
    let bas = std::time::Instant::now();
    p.ok(&["mod", "gozlem"]);
    assert!(bas.elapsed() < std::time::Duration::from_secs(10), "bayat kilit beklendi: {:?}", bas.elapsed());
    assert!(p.ok(&["mod"]).contains("Mod: gozlem"));
    assert!(p.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    assert!(!kilit.exists());
}

/// Kayıt başı dosyası yoksa (eski sürümün kaydı) ya da kayıt dışarıdan değiştiyse zincir yine doğru sürer
#[test]
fn kayit_basi_eski_kayitla_ve_dis_degisiklikle_uyumlu() {
    let ev = Ev::yeni("h3-bas");
    let p = ev.proje("nestjs-temel", "a");
    for m in ["okuma", "gozlem", "okuma"] {
        p.ok(&["mod", m]);
    }
    let k = durum_klasoru(&ev);
    // Eski sürüm: kayıt başı yok
    let _ = std::fs::remove_file(k.join("kayit_bas.json"));
    p.ok(&["mod", "gozlem"]);
    assert!(p.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    // Başka bir araç kayda zincire uygun olmayan bir satır ekledi: yeni satır onu doğrulanmış sanmaz
    let n = satir_sayisi(&p);
    let mut f = std::fs::OpenOptions::new().append(true).open(k.join("kayit.jsonl")).unwrap();
    writeln!(f, "{{\"bozuk\":true}}").unwrap();
    drop(f);
    p.ok(&["mod", "okuma"]);
    let (b, m) = p.cikti(&["kayit", "--dogrula"]);
    assert!(!b && m.contains("bozuk"), "dışarıdan eklenen satır doğrulamada görünmeli: {m}");
    assert!(satir_sayisi(&p) <= n + 2);
}

/// Santralde iki üye, her biri kendi projesinde üretilmiş token'la: virgülle verilen token'lar doğru üyeye gider
#[test]
fn santralde_uye_basina_token() {
    let ev = Ev::yeni("h3-santral");
    let a = ev.proje("nestjs-temel", "a");
    let b = ev.proje("fastapi-temel", "b");
    let ta = a.token("santral", &["*:oku"], &[]);
    let tb = b.token("santral", &["*:oku"], &[]);
    a.ok(&["santral", "ekle", a.klasor.join(".sezgi").to_str().unwrap(), "--ad", "kutuphane"]);
    a.ok(&["santral", "ekle", b.klasor.join(".sezgi").to_str().unwrap(), "--ad", "kitapci"]);
    let iste = |token: &str, araclar: &[(&str, serde_json::Value)]| -> Vec<String> {
        let mut p = a.komut(&["mcp", "--santral"]).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        let mut v = vec![];
        for (i, (ad, arg)) in araclar.iter().enumerate() {
            let istek = serde_json::json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": ad, "arguments": arg } });
            writeln!(stdin, "{istek}").unwrap();
            stdin.flush().unwrap();
            let c: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
            v.push(c["result"]["content"][0]["text"].as_str().unwrap().to_string());
        }
        let _ = p.kill();
        let _ = p.wait();
        v
    };
    let ikisi = format!("{ta},{tb}");
    let c = iste(&ikisi, &[("kimsin", serde_json::json!({})), ("ara", serde_json::json!({ "sorgu": "kitap" }))]);
    assert!(!c[0].contains("Token geçersiz"), "{}", c[0]);
    assert!(c[0].contains("## kutuphane") && c[0].contains("## kitapci") && c[0].matches("Uygulama:").count() == 2, "{}", c[0]);
    assert!(c[1].contains("[kutuphane]") && c[1].contains("[kitapci]"), "iki üyede de arama: {}", c[1]);
    // Yalnızca bir üyenin token'ı: öteki üye açıkça ve anlaşılır biçimde gizlenir
    let c = iste(&ta, &[("kimsin", serde_json::json!({})), ("ara", serde_json::json!({ "sorgu": "kitap" }))]);
    assert!(c[0].contains("## kitapci") && c[0].contains("token") && c[0].contains("sezgi token olustur"), "{}", c[0]);
    assert_eq!(c[0].matches("Uygulama:").count(), 1, "{}", c[0]);
    assert!(c[1].contains("[kutuphane]") && !c[1].lines().any(|l| l.starts_with("[kitapci] ") && l.contains('/')), "gizli üyenin kapıları görünmemeli: {}", c[1]);
}

/// `canli kur` ikinci kez aynı token adıyla çalıştırılınca hiçbir şeyi değiştirmeden durur; başka adla tamamlanır
#[test]
fn canli_kur_tekrar_calistirilabilir() {
    let ev = Ev::yeni("h3-kur");
    let p = ev.proje("nestjs-temel", "a");
    let (u1, u2) = (Uygulama::yeni(), Uygulama::yeni());
    p.ok(&["canli", "kur", "--hedef", &u1.adres(), "--mod", "okuma", "--izin", "kitap.*:oku"]);
    assert!(p.ok(&["mod"]).contains(&u1.adres()));
    let (b, m) = p.cikti(&["canli", "kur", "--hedef", &u2.adres(), "--mod", "okuma", "--izin", "kitap.*:oku"]);
    assert!(!b, "aynı adlı geçerli token varken kurulum başarılı sayılmamalı: {m}");
    assert!(m.contains("yapay-zeka") && m.contains("--token-ad"), "neden ve çözüm söylenmeli: {m}");
    let durum = p.ok(&["mod"]);
    assert!(durum.contains(&u1.adres()) && !durum.contains(&u2.adres()), "yarıda kalan kurulum hedefi değiştirmemeli: {durum}");
    // Geçersiz izin de hiçbir şeyi değiştirmeden durdurur
    let (b, _) = p.cikti(&["canli", "kur", "--hedef", &u2.adres(), "--mod", "okuma", "--izin", "kitap.*:hepsi", "--token-ad", "ikinci"]);
    assert!(!b);
    assert!(p.ok(&["mod"]).contains(&u1.adres()));
    // Başka adla tamamlanır
    p.ok(&["canli", "kur", "--hedef", &u2.adres(), "--mod", "okuma", "--izin", "kitap.*:oku", "--token-ad", "ikinci"]);
    assert!(p.ok(&["mod"]).contains(&u2.adres()));
    assert!(p.ok(&["token", "listele"]).contains("ikinci"));
}
