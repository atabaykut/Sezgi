//! W3 graf tamamlama ve varlık eşlemesi kanıtları:
//! - Graf: `field:` düğümleri (tablo sütunları ve girdi alanları), `rule:` (doğrulama kuralları; davranış özetinin
//!   VALIDATES hedefleriyle aynı "rule:dosya:satır" kimliği), `env:` (koşullu rota kaydının ortam koşulu) ve DEPENDS_ON
//!   (NestJS modül grafiği, FastAPI router içerme), tablo kimliği eşlemesi (Prisma @@map / @map, SQLModel __tablename__).
//!   Haritanın kendisi (kapilar.json) değişmez.
//! - Varlık eşlemesi: açık dış kimlik (bir uygulamanın kodu ötekine onun alanıyla istek atıyor) SOURCE_CODE, tek anlamlıysa
//!   certain; gözlenen veri eşleşmesi (`sezgi santral varlik-gozle`): yalnızca test kopyalarında, insanın oturumuyla,
//!   değerler bellekte eşleşir, yalnızca oran saklanır (observed). Kanıt deposunda örnek değer bulunmaz.
//! - Öneri motoru SQLModel alanlarını da görür; santralde `varlik` ile etki tek anlamlıysa uygulamasız çözülür.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

fn oku(p: &Proje, ad: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join(ad)).unwrap()).unwrap()
}

fn kenar_var(g: &Value, kaynak: &str, iliski: &str, hedef: &str) -> bool {
    g["kenarlar"].as_array().unwrap().iter().any(|e| e["kaynak"] == kaynak && e["iliski"] == iliski && e["hedef"] == hedef)
}

fn dugum<'a>(g: &'a Value, id: &str) -> Option<&'a Value> {
    g["dugumler"].as_array().unwrap().iter().find(|d| d["id"] == id)
}

fn onerme<'a>(k: &'a Value, varlik: &str, yuklem: &str) -> &'a Value {
    k["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).unwrap_or_else(|| panic!("önerme yok: {varlik} {yuklem}"))
}

#[test]
fn graf_alan_kural_tablo_ve_modul() {
    let ev = Ev::yeni("w3g-graf");
    let p = ev.proje("w3g_rezervasyon", "rez");
    let (g, k, h) = (oku(&p, "graf.json"), oku(&p, "kanit.json"), oku(&p, "kapilar.json"));
    // Harita biçimi aynı: yeni bilgi yalnızca kanıt deposuna ve grafa girer
    for ek in ["moduller", "veri_modelleri", "routerlar"] {
        assert!(h["ozet"].get(ek).is_none(), "kapilar.json değişmemeli: {ek}");
    }
    // Tablo sütunları: field:Model.alan; gerçek sütun adı @map'ten, tablo adı @@map'ten
    let f = dugum(&g, "field:Misafir.pasaportNo").unwrap_or_else(|| panic!("sütun düğümü yok"));
    assert_eq!(f["tur"], "field");
    let o = onerme(&k, "field:Misafir.pasaportNo", "VAR");
    assert_eq!((o["deger"]["tablo"].as_str(), o["deger"]["sutun"].as_str()), (Some("misafirler"), Some("pasaport_no")), "{o}");
    assert_eq!(o["hesap"]["guven"], "certain");
    let t = onerme(&k, "db:Rezervasyon", "TABLO");
    assert_eq!((t["deger"]["tablo"].as_str(), t["hesap"]["guven"].as_str()), (Some("rezervasyonlar"), Some("certain")), "{t}");
    // Varsayılan tablo adı: model adı (Prisma)
    assert!(dugum(&g, "field:Rezervasyon.rezervasyonNo").is_some());
    // Girdi alanı ve doğrulama kuralı: işlem -> rule -> alan; kural kimliği "rule:dosya:satır"
    let api = "api:PATCH /api/rezervasyonlar/:id";
    let girdi = "field:RezervasyonGuncelleDto.transferNotu";
    assert!(dugum(&g, girdi).is_some(), "girdi alanı düğümü yok");
    let kural = g["kenarlar"].as_array().unwrap().iter().find(|e| e["iliski"] == "VALIDATES" && e["hedef"] == girdi).unwrap_or_else(|| panic!("alanı doğrulayan kural yok"))
        ["kaynak"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(kural.starts_with("rule:src/rezervasyon/dto/rezervasyon.dto.ts:"), "{kural}");
    assert!(kural.rsplit(':').next().unwrap().parse::<usize>().is_ok(), "{kural}");
    assert!(kenar_var(&g, api, "VALIDATES", &kural), "işlem kuralı uygular");
    let kurallar = onerme(&k, &kural, "KURAL")["deger"]["kurallar"].to_string();
    assert!(kurallar.contains("IsString") && kurallar.contains("MaxLength(200)"), "{kurallar}");
    // Modül grafiği: işlem kendi modülüne, kök modül içe aktardığına bağlı
    assert!(kenar_var(&g, "api:GET /api/rezervasyonlar", "DEPENDS_ON", "mod:RezervasyonModule"), "{}", g["kenarlar"]);
    assert!(kenar_var(&g, "mod:AppModule", "DEPENDS_ON", "mod:RezervasyonModule"));
    assert_eq!(dugum(&g, "mod:RezervasyonModule").unwrap()["tur"], "mod");
}

#[test]
fn graf_fastapi_router_ortam_ve_sqlmodel() {
    let ev = Ev::yeni("w3g-fastapi");
    let p = ev.proje("w3g_otel", "otel");
    let (g, k) = (oku(&p, "graf.json"), oku(&p, "kanit.json"));
    // SQLModel tablosu: __tablename__ ve sütunlar
    let t = onerme(&k, "db:OdaKaydi", "TABLO");
    assert_eq!(t["deger"]["tablo"], "oda_kayitlari", "{t}");
    let s = onerme(&k, "field:OdaKaydi.pasaport_no", "VAR");
    assert_eq!(s["deger"]["tip"], "str", "{s}");
    assert!(dugum(&g, "field:OdaKaydi.id").is_some());
    // Router içerme: işlem router'ına, router uygulamaya bağlı
    let api = "api:GET /api/odalar/";
    assert!(kenar_var(&g, api, "DEPENDS_ON", "mod:app/routes/odalar.py:router"), "{}", g["kenarlar"]);
    assert!(kenar_var(&g, "mod:app/routes/odalar.py:router", "DEPENDS_ON", "mod:app/main.py:app"));
    // Koşullu kayıt: ortam koşulu düğümü ve DEPENDS_ON
    let env = "env:settings.ORTAM == \"local\"";
    assert_eq!(dugum(&g, env).unwrap_or_else(|| panic!("ortam düğümü yok: {}", g["dugumler"]))["tur"], "env");
    assert!(kenar_var(&g, "api:POST /api/deneme/sifirla", "DEPENDS_ON", env));
    assert!(!kenar_var(&g, api, "DEPENDS_ON", env), "koşulsuz işlem ortama bağlı değil");
    // Eski örnek: koşul `if settings.DEBUG:` (fastapi-temel)
    let q = ev.proje("fastapi-temel", "ft");
    let g = oku(&q, "graf.json");
    assert!(kenar_var(&g, "api:POST /api/gizli/kullanici", "DEPENDS_ON", "env:settings.DEBUG"), "{}", g["kenarlar"]);
}

fn santral(ev: &Ev, ornekler: &[(&str, &str)]) -> Vec<Proje> {
    let mut v = vec![];
    for (ornek, ad) in ornekler {
        let p = ev.proje(ornek, ad);
        p.ok(&["santral", "ekle", ".sezgi", "--ad", ad]);
        v.push(p);
    }
    v
}

#[test]
fn oneri_motoru_sqlmodel_alanlarini_gorur() {
    let ev = Ev::yeni("w3g-oneri");
    let p = santral(&ev, &[("w3g_rezervasyon", "rezervasyon"), ("w3g_otel", "otel")]);
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let o = v["oneriler"].as_array().unwrap();
    assert!(
        o.iter().any(|x| {
            let (a, b) = (x["a"].as_str().unwrap(), x["b"].as_str().unwrap());
            [a, b].contains(&"rezervasyon:Misafir.pasaportNo") && [a, b].contains(&"otel:OdaKaydi.pasaport_no") && x["guven"] == "inferred"
        }),
        "SQLModel alanı (yazım farkıyla) önerilmeli: {v:#}"
    );
    p[0].ok(&["santral", "bagla-varlik", "rezervasyon:Misafir.pasaportNo", "otel:OdaKaydi.pasaport_no"]);
}

#[test]
fn acik_dis_kimlik_kaynak_kodundan() {
    let ev = Ev::yeni("w3g-dis");
    let p = santral(&ev, &[("w3g_rezervasyon", "rezervasyon"), ("w3g_transfer", "transfer")]);
    // Transfer servisi rezervasyon uygulamasına, transferin rezervasyonRef değeriyle istek atıyor:
    // GET /api/rezervasyonlar/:rezervasyonNo  ←  `${veri.rezervasyonRef}`
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let e = v["eslemeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            let (a, b) = (e["a"].as_str().unwrap(), e["b"].as_str().unwrap());
            [a, b].contains(&"rezervasyon:Rezervasyon.rezervasyonNo") && [a, b].contains(&"transfer:Transfer.rezervasyonRef")
        })
        .unwrap_or_else(|| panic!("açık dış kimlik eşlemesi yok: {v:#}"))
        .clone();
    assert_eq!(e["guven"], "certain", "tek anlamlı veri akışı: {e:#}");
    let k = e["kanitlar"].as_array().unwrap().iter().find(|k| k["kaynak_turu"] == "SOURCE_CODE").unwrap_or_else(|| panic!("{e:#}"));
    assert!(k["konum"][0].as_str().unwrap().starts_with("transfer:src/transfer/transfer.service.ts:"), "{k}");
    // İnsan da tanımlarsa güven yine certain (kaynak kanıtı var)
    p[0].ok(&["santral", "bagla-varlik", "rezervasyon:Rezervasyon.rezervasyonNo", "transfer:Transfer.rezervasyonRef"]);
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let n = v["eslemeler"].as_array().unwrap().iter().filter(|e| e["a"].as_str().unwrap().contains("rezervasyonNo") || e["b"].as_str().unwrap().contains("rezervasyonNo")).count();
    assert_eq!(n, 1, "aynı eşleme tek önerme: {v:#}");
    // Kaynak değişip dış kimlik akışı kalkınca kaynak kanıtı düşer
    let s = p[1].klasor.join("src").join("transfer").join("transfer.service.ts");
    let m = std::fs::read_to_string(&s).unwrap().replace("${veri.rezervasyonRef}", "x");
    std::fs::write(&s, m).unwrap();
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let e = v["eslemeler"].as_array().unwrap().iter().find(|e| e["a"].as_str().unwrap().contains("rezervasyonNo") || e["b"].as_str().unwrap().contains("rezervasyonNo")).unwrap();
    assert_ne!(e["guven"], "certain", "kaynak kanıtı düşünce insan tanımı tek başına certain değil: {e:#}");
}

/// Yolu verilen listeyi dönen sahte uygulama; istekleri sayar
fn liste_uygulamasi(yol: &'static str, govde: Arc<Mutex<String>>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let gelen = Arc::new(Mutex::new(vec![]));
    let g2 = gelen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            if r.read_line(&mut ilk).is_err() {
                continue;
            }
            let mut boy = 0usize;
            loop {
                let mut b = String::new();
                r.read_line(&mut b).unwrap();
                if b.trim().is_empty() {
                    break;
                }
                if let Some(v) = b.to_ascii_lowercase().strip_prefix("content-length:") {
                    boy = v.trim().parse().unwrap();
                }
            }
            let mut x = vec![0u8; boy];
            r.read_exact(&mut x).unwrap();
            g2.lock().unwrap().push(ilk.clone());
            let cevap = if ilk.contains(yol) { govde.lock().unwrap().clone() } else { "[]".into() };
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len());
        }
    });
    (port, gelen)
}

fn dosyalarda_ara(d: &std::path::Path, aranan: &[&str]) -> Vec<String> {
    let mut v = vec![];
    for g in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = g.path();
        if p.is_dir() {
            v.extend(dosyalarda_ara(&p, aranan));
        } else if let Ok(b) = std::fs::read(&p) {
            let m = String::from_utf8_lossy(&b);
            for a in aranan {
                if m.contains(a) {
                    v.push(format!("{} ⊃ {a}", p.display()));
                }
            }
        }
    }
    v
}

#[test]
fn gozlenen_eslesme_yalniz_oran_saklar() {
    let ev = Ev::yeni("w3g-gozle");
    let p = santral(&ev, &[("w3g_rezervasyon", "rezervasyon"), ("w3g_transfer", "transfer")]);
    let rez_govde =
        Arc::new(Mutex::new(r#"[{"id":"1","rezervasyonNo":"RZ-GIZLI-001"},{"id":"2","rezervasyonNo":"RZ-GIZLI-002"},{"id":"3","rezervasyonNo":"RZ-GIZLI-003"}]"#.to_string()));
    let tr_govde = Arc::new(Mutex::new(
        r#"{"veri":[{"id":"t1","rezervasyonRef":"RZ-GIZLI-001"},{"id":"t2","rezervasyonRef":"RZ-GIZLI-002"},{"id":"t3","rezervasyonRef":"BASKA-GIZLI-9"}]}"#.to_string(),
    ));
    let (rp, rgelen) = liste_uygulamasi("/api/rezervasyonlar", rez_govde.clone());
    let (tp, _) = liste_uygulamasi("/api/transferler", tr_govde.clone());
    for (q, port, oturum) in [(&p[0], rp, "OTURUM-REZ"), (&p[1], tp, "OTURUM-TR")] {
        q.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
        q.ok(&["mod", "okuma"]);
        q.oturum(oturum);
    }
    let esleme = "rezervasyon:Rezervasyon.rezervasyonNo=transfer:Transfer.rezervasyonRef";
    // İnsan onayı olmadan (terminal yok, --evet yok) başlamaz
    let (ok, m) = p[0].cikti(&["santral", "varlik-gozle", esleme, "--ornek", "10"]);
    assert!(!ok && rgelen.lock().unwrap().is_empty(), "{m}");
    let m = p[0].ok(&["santral", "varlik-gozle", esleme, "--ornek", "10", "--evet"]);
    assert!(m.contains("2/3") || m.contains("2 / 3"), "{m}");
    assert!(!m.contains("RZ-GIZLI") && !m.contains("BASKA-GIZLI"), "değer ekrana da yazılmaz: {m}");
    assert!(rgelen.lock().unwrap().iter().any(|l| l.starts_with("GET /api/rezervasyonlar")), "insanın oturumuyla okundu");
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let e = v["eslemeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["a"].as_str().unwrap().contains("rezervasyonNo") || e["b"].as_str().unwrap().contains("rezervasyonNo"))
        .unwrap()
        .clone();
    let k = e["kanitlar"].as_array().unwrap().iter().find(|k| k["kaynak_turu"] == "RUNTIME_TRACE").unwrap_or_else(|| panic!("{e:#}"));
    assert_eq!(k["katki"], "observed", "{k}");
    // Değerler hiçbir depoya girmez: kanıt, kayıt, durum, harita — örnek değer bayt araması 0
    let bulunan = dosyalarda_ara(&ev.kok, &["RZ-GIZLI", "BASKA-GIZLI"]);
    assert!(bulunan.is_empty(), "değer diske yazılmış: {bulunan:?}");
    // Eşleşme yoksa (oran düşük) gözlem çelişkidir: güven en fazla yorum
    *tr_govde.lock().unwrap() = r#"[{"rezervasyonRef":"YOK-1"},{"rezervasyonRef":"YOK-2"}]"#.into();
    p[0].ok(&["santral", "varlik-gozle", esleme, "--ornek", "10", "--evet"]);
    let v: Value = serde_json::from_str(&p[0].ok(&["santral", "varliklar", "--json"])).unwrap();
    let e = v["eslemeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["a"].as_str().unwrap().contains("rezervasyonNo") || e["b"].as_str().unwrap().contains("rezervasyonNo"))
        .unwrap()
        .clone();
    assert!(["inferred", "unknown"].contains(&e["guven"].as_str().unwrap()), "çelişen gözlem: {e:#}");
    // Test kopyası olmayan hedefte gözlem yok
    p[1].ok(&["hedef", &format!("http://127.0.0.1:{tp}")]);
    let (ok, m) = p[0].cikti(&["santral", "varlik-gozle", esleme, "--ornek", "10", "--evet"]);
    assert!(!ok && m.contains("test kopya"), "{m}");
}

#[test]
fn santral_varlik_uygulamasiz_cozulur() {
    let ev = Ev::yeni("w3g-coz");
    let p = santral(&ev, &[("w3g_rezervasyon", "rezervasyon"), ("w3g_transfer", "transfer")]);
    let mut c = p[0].komut(&["mcp", "--santral"]).env("SEZGI_TOKEN", "").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = c.stdin.take().unwrap();
    for (i, a) in [json!({ "varlik": "db:Transfer" }), json!({ "varlik": "db:Hayalet" })].iter().enumerate() {
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": "etki", "arguments": a } })).unwrap();
    }
    drop(stdin);
    let o = c.wait_with_output().unwrap();
    let cevaplar: Vec<Value> = String::from_utf8_lossy(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (t, hata) = (cevaplar[0]["result"]["content"][0]["text"].as_str().unwrap(), cevaplar[0]["result"]["isError"] == true);
    assert!(!hata && t.contains("POST /api/transferler"), "tek uygulamada olan varlık uygulamasız çözülür: {t}");
    assert_eq!(cevaplar[1]["result"]["isError"], true, "{}", cevaplar[1]);
}
