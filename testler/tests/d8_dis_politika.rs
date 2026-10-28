//! D8 dış etki politikası: dış sisteme yazan ya da geri alınamaz yazma onaysız gitmez. Aynı işlem dış çağrı olmadan
//! (nestjs-temel) onaysız gidiyordu; tek fark yorum servisindeki giden POST. Kart, canlı mod ve Ayna aynı gerekçeyi
//! taşır; Ayna'nın 10, 12 ve 13. soruları bu verinin özetidir.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::Stdio;

#[test]
fn dis_etkili_yazma_onaya_duser_digerleri_degismez() {
    let ev = Ev::yeni("d8-dis-politika");
    let u = Uygulama::yeni();
    let p = ev.proje("d8_dis_nest", "a");
    p.kur(&u, "OTURUM-ALICE");
    let kart = p.kart();
    let bul = |y: &str, yol: &str| kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == y && o["entry"]["path"] == yol).unwrap().clone();
    let yorum = bul("POST", "/api/v1/yorumlar");
    assert_eq!(yorum["stamp"], "certain", "{yorum}");
    assert_eq!(yorum["requires_approval"], true, "dış etkili yazma kartta da onay ister: {yorum}");
    assert!(yorum["x-dna"]["approval_reason"].as_str().unwrap().contains("dışına etki"), "{yorum}");
    assert_eq!(bul("GET", "/api/v1/kitaplar/:id")["requires_approval"], false);
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("dışına etki"), "dış etkili yazma onaysız gitmez: {s}");
    assert_eq!(u.sayi(), 0, "istek uygulamaya gitmedi");
}

#[test]
fn ayna_dis_geri_ve_onkosul_sorulari() {
    let ev = Ev::yeni("d8-dis-ayna");
    let p = ev.proje("d8_dis_express", "a");
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let (port, anahtar) = loop {
        let mut satir = String::new();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port: u16 = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar: String = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            break (port, anahtar);
        }
    };
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET /api/sorular HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Sezgi-Anahtar: {anahtar}\r\n\r\n").unwrap();
    let mut cevap = String::new();
    let _ = s.read_to_string(&mut cevap);
    let _ = c.kill();
    let _ = c.wait();
    let govde = cevap.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    let govde = &govde[govde.find('{').unwrap_or(0)..govde.rfind('}').map(|i| i + 1).unwrap_or(govde.len())];
    let b: Value = serde_json::from_str(govde).unwrap_or_else(|e| panic!("{e}: {govde}"));
    let soru = |id: &str| b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    // 12: dış sistemler hedef türüne göre; hiçbiri "izlenmez" demez
    let d = soru("dis");
    let metin = d.to_string();
    assert!(metin.contains("api.odeme.example.com") && metin.contains("ortam.UCUS_API_URL"), "{d:#}");
    // Sabit adres zincirden ulaşıldığı için yorum; çözülemeyen adres bilinmiyor
    assert!(d["maddeler"].as_array().unwrap().iter().any(|m| m["guven"] == "inferred" && m["metin"].as_str().unwrap().contains("sabit adresli")), "{d:#}");
    assert!(d["maddeler"].as_array().unwrap().iter().any(|m| m["guven"] == "unknown" && m["metin"].as_str().unwrap().contains("çözülemeyen")), "{d:#}");
    assert!(!metin.contains("izlenmez"), "{d:#}");
    // 13: karar başına işlemler, en fazla yorum
    let g = soru("geri");
    assert!(g.to_string().contains("geri alınamaz") && g.to_string().contains("DELETE /api/siparisler/:id"), "{g:#}");
    assert!(g["maddeler"].as_array().unwrap().iter().all(|m| m["guven"] != "certain"), "{g:#}");
    // 10: red kodu bilinmeyen koşul da ön koşul olarak sayılır
    let o = soru("onkosul").to_string();
    assert!(o.contains("ön koşul") && o.contains("PUT /api/siparisler/:id"), "{o}");
    // 14: onay sorusu dış etkili yazmayı ayrı sayar
    assert!(soru("onay").to_string().contains("uygulamanın dışına etki ediyor"), "{}", soru("onay"));
}

#[test]
fn santral_baska_uygulamanin_tabanina_giden_cagri_bagdir() {
    let ev = Ev::yeni("d8-dis-santral");
    let a = ev.proje("d8_dis_express", "siparis");
    let b = ev.proje("d8_dis_ucus", "ucus");
    a.ok(&["santral", "ekle", ".sezgi", "--ad", "siparis"]);
    b.ok(&["santral", "ekle", ".sezgi", "--ad", "ucus"]);
    let c = a.ok(&["santral", "baglar", "--json"]);
    let v: Value = serde_json::from_str(&c[c.find('{').unwrap()..]).unwrap();
    let bag = v["baglar"].as_array().unwrap().iter().find(|x| x["tur"] == "dis_cagri").unwrap_or_else(|| panic!("dış çağrı bağı yok: {v:#}"));
    // `${process.env.UCUS_API_URL}/flights/${no}`: ortam değişkeninin adı üyenin adını taşıyor, yol kapıya denk geliyor
    assert_eq!(bag["kaynak"], "siparis");
    assert_eq!(bag["kapi"], "ucus: GET /flights/:no", "{bag}");
    assert_eq!(bag["hedef"], "external:ortam.UCUS_API_URL", "{bag}");
    assert!(bag["kaynak_islemler"].as_array().unwrap().iter().any(|i| i == "GET /api/ucuslar/:no"), "{bag}");
}
