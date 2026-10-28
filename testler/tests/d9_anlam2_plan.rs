//! D9 anlam2: çok adımlı görev planında fiil-nesne eşlemesi. Her parçada fiil ve nesne ayrı bulunur; nesne kaynağa,
//! fiil işlemin eylemine eşlenir. Okuma fiili (listele, göster, bul, kontrol et) yazma işlemine eşlenmez; fiille
//! uyuşmayan işlem seçilmez; karşılığı yoksa adım dürüstçe "bulunamadı" der. Yönelme hâlindeki varlık ("bütçeye")
//! hedef kaynaktır: "bütçeye kategori ekle" iç içe işleme gider.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::json;
use std::io::Write;
use std::process::Stdio;

/// İngilizce MCP oturumunda tek bir `search` task=true çağrısının metni
fn ingilizce_ara(p: &Proje, q: &str) -> String {
    let mut c = p.komut(&["mcp", ".sezgi", "--lang", "en"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "search", "arguments": { "query": q, "task": true } } });
    writeln!(c.stdin.take().unwrap(), "{istek}").unwrap();
    let cikti = String::from_utf8_lossy(&c.wait_with_output().unwrap().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(cikti.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or(&cikti).to_string()
}

/// Plan metninde "n. " ile başlayan adımın işlem satırları
fn adim(t: &str, n: usize) -> String {
    let bas = format!("{n}. ");
    let mut v = vec![];
    let mut icinde = false;
    for l in t.lines() {
        if l.starts_with(&bas) {
            icinde = true;
        } else if icinde && !l.starts_with("   ") {
            break;
        }
        if icinde {
            v.push(l);
        }
    }
    v.join("\n")
}

#[test]
fn duraklat_ve_listele() {
    let ev = Ev::yeni("d9-anlam2-plan-iz");
    let p = ev.proje("d9_anlam2_izleme", "nabiz");
    let mut m = p.mcp("");
    let (t, _) = m.arac("ara", json!({ "sorgu": "list the checks and pause a check", "gorev": true }));
    assert!(adim(&t, 1).contains("GET ") && adim(&t, 1).contains("checks"), "{t}");
    assert!(adim(&t, 2).contains("POST /api/v1/checks/:code/pause"), "{t}");
    // Uygulamada karşılığı olmayan adım: bildirim/ekip işlemi uydurulmaz
    let (t, _) = m.arac("ara", json!({ "sorgu": "pause a check and notify the team", "gorev": true }));
    assert!(t.contains("POST /api/v1/checks/:code/pause"), "{t}");
    assert!(!t.contains("/integrations/:code/test/"), "{t}");
    assert!(t.contains("bulunamadı"), "{t}");
    // Türkçe
    let (t, _) = m.arac("ara", json!({ "sorgu": "check'i duraklat, sonra devam ettir", "gorev": true }));
    assert!(t.contains("POST /api/v1/checks/:code/pause") && t.contains("POST /api/v1/checks/:code/resume"), "{t}");
}

#[test]
fn okuma_fiili_yazma_secmez() {
    let ev = Ev::yeni("d9-anlam2-plan-oku");
    let p = ev.proje("d9_anlam2_kumbara", "kumbara");
    let mut m = p.mcp("");
    let (t, _) = m.arac("ara", json!({ "sorgu": "işlemleri bul ve hesabı sil", "gorev": true }));
    let bul = t.lines().find(|l| l.contains("← \"işlemleri bul\"")).map(|_| ()).is_some();
    assert!(bul, "{t}");
    assert!(!t.contains("POST /api/v1/transactions/ ·"), "okuma fiili oluşturma işlemine eşlendi:\n{t}");
    assert!(t.contains("GET /api/v1/transactions/") || t.contains("POST /api/v1/transactions/search"), "{t}");
    assert!(t.contains("DELETE /api/v1/accounts/:id"), "{t}");
    // Göster: yalnızca okuma
    let (t, _) = m.arac("ara", json!({ "sorgu": "show the budgets and then delete the chat", "gorev": true }));
    assert!(t.contains("GET /api/v1/budgets/") && !t.contains("POST /api/v1/budgets/"), "{t}");
    assert!(t.contains("DELETE /api/v1/chats/:id"), "{t}");
}

#[test]
fn yonelme_hedef_kaynak_ve_ingilizce_ad_sirasi() {
    let ev = Ev::yeni("d9-anlam2-plan-hedef");
    let p = ev.proje("d9_anlam2_kumbara", "kumbara");
    let mut m = p.mcp("");
    let (t, _) = m.arac("ara", json!({ "sorgu": "bütçe oluştur ve bütçeye kategori ekle", "gorev": true }));
    assert!(t.contains("POST /api/v1/budgets/ ·") && t.contains("POST /api/v1/budgets/:id/categories"), "{t}");
    // "hesabı kapat": kapatma işlemi yok; görüntüleme seçilmez, en yakın yazma işlemi açıkça "yakın" diye yazılır
    let (t, _) = m.arac("ara", json!({ "sorgu": "işlemleri listele ve hesabı kapat", "gorev": true }));
    assert!(!t.contains("GET /api/v1/accounts/"), "{t}");
    assert!(t.contains("en yakın") || t.contains("bulunamadı"), "{t}");
    // İngilizce: adım adı "fiil nesne" sırasında
    let t = ingilizce_ara(&p, "create a chat then send a message");
    assert!(t.contains("create chat") && t.contains("send message"), "{t}");
    assert!(t.contains("POST /api/v1/chats/:id/messages"), "{t}");
    assert!(!t.contains("chat create"), "{t}");
}
