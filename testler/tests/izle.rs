//! `sezgi calistir`: kod değişince harita yalnızca değişen dosya yeniden ayrıştırılarak güncellenir; çalışan MCP
//! sunucusu yeni haritayı bir sonraki çağrıda görür.
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

fn ornek(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-izle-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &k);
    k
}

fn endpointler(harita: &Path) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(harita.join("kapilar.json")).unwrap()).unwrap();
    v["endpointler"].as_array().unwrap().iter().map(|e| format!("{} {}", e["yontem"].as_str().unwrap_or(""), e["yol"].as_str().unwrap_or(""))).collect()
}

const YENI_UC: &str = "\n  @Get('izle/yeni-uc')\n  yeniUc() {}\n}\n";

fn uc_ekle(kok: &Path) {
    let f = kok.join("src").join("kitap").join("kitap.controller.ts");
    let m = std::fs::read_to_string(&f).unwrap();
    let i = m.rfind('}').unwrap();
    std::fs::write(&f, format!("{}{}", &m[..i], YENI_UC)).unwrap();
}

#[test]
fn degisiklik_yalnizca_degisen_dosyayi_yeniden_ayristirir() {
    let kok = ornek("artimli");
    let harita = kok.join(".sezgi");
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["calistir", kok.to_str().unwrap(), "--aralik", "100", "--en-fazla", "1"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
    // İlk kurulum bitip izleme başlayana kadar
    for s in satirlar.by_ref() {
        if s.unwrap().contains("İzleniyor") {
            break;
        }
    }
    let once = endpointler(&harita);
    assert!(!once.iter().any(|e| e.contains("yeni-uc")));
    uc_ekle(&kok);
    let guncelleme = satirlar.by_ref().map(|s| s.unwrap()).find(|s| s.starts_with('↻')).expect("güncelleme satırı");
    let durum = p.wait().unwrap();
    assert!(durum.success());
    let sonra = endpointler(&harita);
    assert!(sonra.iter().any(|e| e.contains("izle/yeni-uc")), "{sonra:?}");
    assert_eq!(sonra.len(), once.len() + 1);
    assert!(guncelleme.contains("(+1)") && guncelleme.contains("1 dosya değişti"), "{guncelleme}");
    // Önbellek: yalnızca değişen dosya yeniden ayrıştırıldı
    assert!(guncelleme.contains(" 1 / "), "yalnızca bir dosya yeniden ayrıştırılmalı: {guncelleme}");
    let _ = std::fs::remove_dir_all(&kok);
}

fn mcp_cagir(stdin: &mut std::process::ChildStdin, satirlar: &mut impl Iterator<Item = std::io::Result<String>>, id: u64) -> String {
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": "kimsin", "arguments": {} } });
    writeln!(stdin, "{istek}").unwrap();
    stdin.flush().unwrap();
    let cevap: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
    cevap["result"]["content"][0]["text"].as_str().unwrap().to_string()
}

#[test]
fn mcp_sunucusu_guncellenen_haritayi_gorur() {
    let kok = ornek("mcp");
    let harita = kok.join(".sezgi");
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap()]).output().unwrap().status.success());
    let n = endpointler(&harita).len();
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = p.stdin.take().unwrap();
    let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
    assert!(mcp_cagir(&mut stdin, &mut satirlar, 1).contains(&format!("{n} kapı")));
    // Değişim zamanı çözünürlüğü kaba olabilir: harita en az biraz sonra yeniden yazılsın
    std::thread::sleep(Duration::from_millis(50));
    uc_ekle(&kok);
    let t = Instant::now();
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap()]).output().unwrap().status.success());
    assert!(t.elapsed() < Duration::from_secs(30));
    let metin = mcp_cagir(&mut stdin, &mut satirlar, 2);
    assert!(metin.contains(&format!("{} kapı", n + 1)), "MCP yeni haritayı görmeli: {metin}");
    drop(stdin);
    p.wait().unwrap();
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn deneme_ile_calistirilmaz() {
    let kok = ornek("deneme");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["calistir", kok.to_str().unwrap(), "--deneme"]).output().unwrap();
    assert!(!c.status.success());
    assert!(!kok.join(".sezgi").exists());
    let _ = std::fs::remove_dir_all(&kok);
}
