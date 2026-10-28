//! W2 kaynaksız bağlama, gerçek REA ile (rea-agents@6.7.0 + Node 24.15+). Varsayılan çalıştırmada atlanır (#[ignore]);
//! test görüntüsüne `/rea` (npm i rea-agents@6.7.0) ve `/n/node` bağlanınca: `cargo test --test w2kb_rea -- --ignored`.
//! .NET üyeleri grafa bağlanır: IL çağrı hedefi CALLS (observed), literal SQL'den metot → tablo WRITES (inferred),
//! P/Invoke; etki metottan tabloya ve tetikleyiciyle başka tabloya ulaşır; kart yine hiçbir şeyi yazılabilir saymaz.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const REA: &str = "/rea/node_modules/.bin/rea";
const NODE: &str = "/n/node";

fn kenar<'a>(graf: &'a Value, kaynak: &str, iliski: &str, hedef: &str) -> Option<&'a Value> {
    graf["kenarlar"].as_array().unwrap().iter().find(|k| k["kaynak"] == kaynak && k["iliski"] == iliski && k["hedef"] == hedef).map(|k| &k["guven"])
}

fn calis(args: &[&str], ayar: &Path) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(args)
        .env("SEZGI_REA", REA)
        .env("SEZGI_NODE", NODE)
        .env("APPDATA", ayar)
        .env("XDG_CONFIG_HOME", ayar)
        .env("HOME", ayar)
        .output()
        .unwrap();
    let m = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{args:?}: {m}");
    m
}

#[test]
#[ignore]
fn w2kb_rea_net_uyeleri_grafa_baglanir() {
    if !Path::new(REA).exists() || !Path::new(NODE).exists() {
        eprintln!("REA ya da Node bağlı değil: atlandı");
        return;
    }
    let k = std::env::temp_dir().join(format!("sezgi-w2kb-rea-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    let y = k.join("yayin");
    std::fs::create_dir_all(&y).unwrap();
    for g in std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w2k_otelrez/yayin")).unwrap().flatten() {
        std::fs::copy(g.path(), y.join(g.file_name())).unwrap();
    }
    calis(&["kesfet", y.to_str().unwrap(), "--rea"], &k);
    let c = y.join(".sezgi");
    let graf: Value = serde_json::from_str(&std::fs::read_to_string(c.join("graf.json")).unwrap()).unwrap();
    let f = |m: &str| format!("fn:OtelRez.dll#OtelRez.{m}");
    assert_eq!(kenar(&graf, &f("Program.Main"), "CALLS", &f("IptalKurali.IptalEt")), Some(&json!("observed")), "IL çağrı hedefi");
    assert_eq!(kenar(&graf, &f("Depo.IptalKomutu"), "WRITES", "db:rezervasyon"), Some(&json!("inferred")), "literal SQL → tablo");
    assert_eq!(kenar(&graf, "mod:OtelRez.dll", "CALLS", "fn:e_sqlite3!sqlite3_open"), Some(&json!("observed")), "P/Invoke");
    assert_eq!(kenar(&graf, &f("IptalKurali.IptalEt"), "IMPLEMENTS", "cap:iptalkurali.iptalet"), Some(&json!("inferred")), "addan yetenek");
    // Etki: Main → IptalKomutu → rezervasyon (yazma) → tetikleyici → iptal_kaydi
    let o = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", c.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "etki", "arguments": { "varlik": f("Program.Main") } } });
    use std::io::{BufRead, Write};
    let mut o = o;
    writeln!(o.stdin.take().unwrap(), "{istek}").unwrap();
    let satir = std::io::BufReader::new(o.stdout.take().unwrap()).lines().next().unwrap().unwrap();
    let _ = o.wait();
    let v: Value = serde_json::from_str(&satir).unwrap();
    let etki = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(etki.contains("rezervasyon") && etki.contains("iptal_kaydi") && !etki.contains("Kesin:"), "{etki}");
    let kart: Value = serde_json::from_str(&std::fs::read_to_string(c.join("benlik.json")).unwrap()).unwrap();
    let cap = kart["x-capabilities"].as_array().unwrap().iter().find(|x| x["id"] == "cap:iptalkurali.iptalet").cloned().unwrap();
    assert_eq!((cap["stamp"].as_str(), cap["requires_approval"].as_bool()), (Some("inferred"), Some(true)));
    assert_eq!(kart["operations"], json!([]));
    let d = calis(&["doktor", y.to_str().unwrap(), "--rea"], &k);
    assert!(d.contains("rea-agents 6.7.0 hazır"), "{d}");
}
