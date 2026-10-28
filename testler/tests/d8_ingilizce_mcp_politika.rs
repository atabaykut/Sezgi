//! D8 İngilizce (politika): canlı modun kararı dilden bağımsızdır. Aynı fikstür, aynı token ve aynı istekler Türkçe ve
//! İngilizce MCP sunucusunda aynı sonucu verir: uygulamaya giden istekler, insan onayı bekleyenler ve önizlemede
//! reddedilenler birebir aynı. Önizleme sonucu metinden değil dilden bağımsız karardan okunur.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

/// Bir isteğin politika sonucu: (isError, uygulamaya giden istek sayısındaki artış, onay bekliyor mu)
type Sonuc = (bool, usize, bool);

fn istekler() -> Vec<Value> {
    vec![
        // Bütün bilinen kuralları geçen kesin yazma
        yorum_yaz(),
        // Önizlemenin bir kuralı denetleyemediği yazma (ValidateIf): onay
        kitap_olustur(),
        // Bilinen kuralı çiğneyen yazma: önizlemede reddedilir, gönderilmez
        json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "puan": 99 } }),
        // Silme: her seferinde onay
        json!({ "kapi": "DELETE /api/v1/yorumlar/toplu" }),
        // Okuma
        json!({ "kapi": "GET /api/v1/kitaplar" }),
        // Olmayan kapı
        json!({ "kapi": "POST /yok/boyle" }),
    ]
}

fn kararlar(dil: &str) -> Vec<Sonuc> {
    let ev = Ev::yeni(&format!("d8-ing-politika-{dil}"));
    let p = ev.proje("nestjs-temel", "a");
    let u = Uygulama::yeni();
    p.kur(&u, "OTURUM-ALICE");
    let token = p.token("ajan", &["*:yaz"], &["--tum-islemler"]);
    let mut c = p
        .komut(&["mcp", ".sezgi", "--lang", dil])
        .env("SEZGI_TOKEN", &token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = c.stdin.take().unwrap();
    let mut satirlar = BufReader::new(c.stdout.take().unwrap()).lines();
    let mut sonuc = vec![];
    for (i, a) in istekler().into_iter().enumerate() {
        let once = u.sayi();
        let istek = json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": "cagir", "arguments": a } });
        writeln!(stdin, "{istek}").unwrap();
        stdin.flush().unwrap();
        let v: Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
        let metin = v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
        let bekliyor = metin.contains("Onay kimliği: ") || metin.contains("Approval id: ");
        sonuc.push((v["result"]["isError"] == true, u.sayi() - once, bekliyor));
    }
    let _ = c.kill();
    let _ = c.wait();
    sonuc
}

#[test]
fn turkce_ve_ingilizce_ayni_politika_karari() {
    let tr = kararlar("tr");
    let en = kararlar("en");
    assert_eq!(tr, en, "politika kararı dile bağlı olmamalı");
    // Kararlar anlamlı: en az bir istek gitti, en az biri onay bekliyor, reddedilen gönderilmedi
    assert!(tr.iter().any(|x| x.1 > 0), "{tr:?}");
    assert!(tr.iter().any(|x| x.2), "{tr:?}");
    assert_eq!(tr[2].1, 0, "önizlemede reddedilen istek gönderilmez: {tr:?}");
}
