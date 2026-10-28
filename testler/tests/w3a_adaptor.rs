//! W3-adaptör-tam: göç dosyalarındaki kaskadlar, olay / iş düğümleri ve tetik bağları, router düzeyindeki FastAPI
//! bağımlılıkları, uygulama geneli operationId üretimi (NestJS `operationIdFactory`, FastAPI etiket deseni ve
//! include_router etiketleri), Express `asyncHandler` sarmalı, Vue / Svelte ön yüzlerinde istemci çağrıları ve
//! çalışma alanı paketindeki üretilmiş istemci (oazapfts).
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            kopyala(&p, &hedef.join(g.file_name()));
        } else {
            std::fs::copy(&p, hedef.join(g.file_name())).unwrap();
        }
    }
}

/// Örneği geçici klasöre kopyalar ve `alt` klasöründe init çalıştırır. Dönen: (geçici kök, harita klasörü)
fn haritala(ad: &str, alt: &str) -> (PathBuf, PathBuf) {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let kok = std::env::temp_dir().join(format!("sezgi-w3a-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&ornek(ad), &kok.join("depo"));
    std::fs::create_dir_all(kok.join("ev")).unwrap();
    let uygulama = kok.join("depo").join(alt);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", "--paylasimsiz"])
        .current_dir(&uygulama)
        .env("HOME", kok.join("ev"))
        .env("APPDATA", kok.join("ev"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    (kok, uygulama.join(".sezgi"))
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn kapi<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
}

fn dna<'a>(h: &'a Value, id: &str) -> &'a Value {
    let k = kapi(h, id);
    let yer = format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]);
    &h["ozet"]["dna"][yer.as_str()]
}

fn arac(harita: &Path, ad: &str, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn kaskad_var(h: &Value, ebeveyn: &str, cocuk: &str, dosya: &str) -> bool {
    h["ozet"]["kaskadlar"].as_array().unwrap().iter().any(|k| k[0] == ebeveyn && k[1] == cocuk && k[2].as_str().unwrap_or("").contains(dosya))
}

#[test]
fn w3a_nest_goc_kaskadi_olay_is_zinciri_ve_islem_kimligi() {
    let (kok, harita) = haritala("w3a_nest", "");
    let h = json_oku(&harita.join("kapilar.json"));
    // Kaskadlar: göç dosyasındaki SQL (yeniden adlandırma izlenir), .sql dosyası, tablo sınıfının dekoratörü
    assert!(kaskad_var(&h, "user", "asset", "1700000000000-Initial.ts"), "{:#}", h["ozet"]["kaskadlar"]);
    assert!(kaskad_var(&h, "asset", "album_asset", "1700000000000-Initial.ts"), "{:#}", h["ozet"]["kaskadlar"]);
    assert!(kaskad_var(&h, "asset", "tag_asset", "db/extra.sql"), "{:#}", h["ozet"]["kaskadlar"]);
    assert!(kaskad_var(&h, "asset", "asset_face", "asset-face.table.ts"), "{:#}", h["ozet"]["kaskadlar"]);
    // onUpdate: 'CASCADE' + onDelete: 'SET NULL' kaskad silme değildir
    assert!(!kaskad_var(&h, "asset", "album", ""), "{:#}", h["ozet"]["kaskadlar"]);
    // DELETE /api/assets: force'a göre iki olay; kalıcı silme olayından iş zincirine
    let d = dna(&h, "DELETE /api/assets");
    let tetikler: Vec<String> = d["tetikler"].as_array().unwrap().iter().map(|t| format!("{}:{}", t["tur"].as_str().unwrap(), t["ad"].as_str().unwrap())).collect();
    assert!(tetikler.contains(&"event:AssetDeleteAll".to_string()) && tetikler.contains(&"event:AssetTrashAll".to_string()), "{d:#}");
    let ol = &h["ozet"]["olaylar"];
    let ad_listesi = |id: &str, alan: &str| -> Vec<String> {
        ol[id][alan]
            .as_array()
            .unwrap_or_else(|| panic!("{id} yok: {ol:#}"))
            .iter()
            .map(|t| format!("{}:{}", t["tur"].as_str().unwrap_or(""), t["ad"].as_str().unwrap_or("")))
            .collect()
    };
    assert_eq!(ad_listesi("event:AssetDeleteAll", "tetikler"), ["job:AssetEmptyTrash"]);
    assert_eq!(ad_listesi("job:AssetEmptyTrash", "tetikler"), ["job:AssetDelete"]);
    assert!(ad_listesi("job:AssetDelete", "tetikler").contains(&"job:FileDelete".to_string()));
    assert_eq!(ol["job:AssetDelete"]["dinleyiciler"][0]["fonksiyon"], "AssetService.handleAssetDeletion");
    // İş, üst sınıftaki depo alanı üzerinden asset tablosunu siler; kaskadla dolaylı yazılanlar
    assert_eq!(ol["job:AssetDelete"]["silinen"], json!(["asset"]), "{:#}", ol["job:AssetDelete"]);
    let dolayli: Vec<&str> = ol["job:AssetDelete"]["dolayli_yazar"].as_array().unwrap().iter().filter_map(|x| x["tablo"].as_str()).collect();
    for t in ["asset_face", "album_asset", "tag_asset"] {
        assert!(dolayli.contains(&t), "{t}: {dolayli:?}");
    }
    // Kanıt deposu ve etki: işlem -> olay -> iş -> iş
    let depo = json_oku(&harita.join("kanit.json"));
    let bag = |v: &str, hdf: &str| depo["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"] == v && o["yuklem"] == "TRIGGERS" && o["hedef"] == hdf).cloned();
    let o = bag("api:DELETE /api/assets", "event:AssetDeleteAll").expect("işlem -> olay TRIGGERS");
    assert_eq!(o["hesap"]["guven"], "inferred");
    assert!(bag("event:AssetDeleteAll", "job:AssetEmptyTrash").is_some() && bag("job:AssetEmptyTrash", "job:AssetDelete").is_some());
    let e = arac(&harita, "etki", json!({ "kapi": "DELETE /api/assets" }));
    assert!(e.contains("AssetDelete") && e.contains("AssetEmptyTrash"), "{e}");
    let t = arac(&harita, "tani", json!({ "kapi": "DELETE /api/assets" }));
    assert!(t.contains("tetikler: event:AssetDeleteAll → TrashService.onAssetsDelete → job:AssetEmptyTrash"), "{t}");
    // Kart: dahili düğümler işlem değildir (x-nodes), bağlar x-relationships
    let kart = json_oku(&harita.join("benlik.json"));
    let ops = kart["operations"].as_array().unwrap();
    assert_eq!(ops.len(), h["endpointler"].as_array().unwrap().iter().filter(|k| k["ana"] == true).count(), "iş ve olaylar işlem sayılmaz");
    let dugum = kart["x-nodes"].as_array().unwrap().iter().find(|n| n["id"] == "job:AssetDelete").expect("x-nodes job:AssetDelete");
    assert_eq!(dugum["type"], "job");
    assert_eq!(dugum["stamp"], "inferred");
    assert!(kart["x-relationships"].as_array().unwrap().iter().any(|r| r["to"] == "event:AssetDeleteAll" && r["kind"] == "triggers"));
    // operationIdFactory: (controllerKey, methodKey) => methodKey
    let k = kapi(&h, "DELETE /api/assets");
    assert_eq!(h["ozet"]["islem_kimlikleri"][format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]).as_str()], "deleteAssets");
    let openapi = json_oku(&harita.join("openapi.json"));
    assert_eq!(openapi["paths"]["/api/assets"]["delete"]["operationId"], "deleteAssets", "{:#}", openapi["paths"]["/api/assets"]);
    // Yedekten geri dönme (start-restore) silmenin tersi değildir
    assert_eq!(dna(&h, "DELETE /api/admin/database-backups")["geri_alinabilir"], "bilinmiyor");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn w3a_fastapi_router_bagimliligi_arka_plan_isi_etiket_kimligi() {
    let (kok, harita) = haritala("w3a_fastapi", "");
    let h = json_oku(&harita.join("kapilar.json"));
    let d = dna(&h, "DELETE /api/varliklar/:id");
    // APIRouter(dependencies=[Depends(admin_iste)]) zincire katılır
    let r = d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == 403).unwrap_or_else(|| panic!("router bağımlılığındaki 403 yok: {d:#}"));
    assert_eq!(r["kaynak"], "bagimlilik");
    assert!(r["kanit"].as_str().unwrap().contains("deps.py"), "{r}");
    // BackgroundTasks.add_task -> iş
    assert!(d["tetikler"].as_array().unwrap().iter().any(|t| t["tur"] == "job" && t["ad"] == "eposta_gonder"), "{d:#}");
    // include_router(tags=["depo"]) etiketi rotanın etiketlerinin başına geçer: "depo-sil"
    let k = kapi(&h, "DELETE /api/varliklar/:id");
    assert_eq!(h["ozet"]["islem_kimlikleri"][format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]).as_str()], "depo-sil", "{:#}", h["ozet"]["islem_kimlikleri"]);
    // Alembic göçündeki kaskadlar
    assert!(kaskad_var(&h, "varlik", "etiket", "0001_baslangic.py"), "{:#}", h["ozet"]["kaskadlar"]);
    assert!(kaskad_var(&h, "varlik", "yorum", "0001_baslangic.py"), "{:#}", h["ozet"]["kaskadlar"]);
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn w3a_express_async_handler_govdesi() {
    let (kok, harita) = haritala("w3a_express", "");
    let h = json_oku(&harita.join("kapilar.json"));
    let k = kapi(&h, "POST /api/kullanicilar");
    let alanlar: Vec<&str> = k["govde"].as_array().unwrap().iter().flat_map(|g| g["sema"]["alanlar"].as_object().into_iter().flat_map(|a| a.keys().map(String::as_str))).collect();
    for a in ["ad", "eposta", "sifre"] {
        assert!(alanlar.contains(&a), "asyncHandler içindeki req.body alanı {a}: {k:#}");
    }
    // Yorumdaki "from our database" tablo sayılmaz
    assert!(k["veri"].is_null() || !k["veri"]["okur"].as_array().unwrap().iter().any(|t| t == "our"), "{k:#}");
    assert!(dna(&h, "POST /api/kullanicilar")["hatalar"].as_array().unwrap().iter().any(|x| x["durum"] == 400 && x["kosul"] == "var_mi"));
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn w3a_svelte_vue_ve_calisma_alani_istemcisi() {
    let (kok, harita) = haritala("w3a_svelte", "server");
    let w = json_oku(&harita.join("web-baglari.json"));
    let cagrilar = w["istemci_cagrilari"].as_array().unwrap();
    let bul = |id: &str| cagrilar.iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("{id} yok: {w:#}"));
    // Çalışma alanı paketindeki oazapfts istemcisi (yol sunucunun genel önekisiz)
    let sil = bul("client:varliklariSil");
    assert_eq!(sil["api"], "DELETE /api/varliklar", "{sil}");
    assert!(sil["kanit"]["dosya"].as_str().unwrap().contains("packages/sdk/src/fetch-client.ts"), "{sil}");
    assert!(!sil["kanit"]["dosya"].as_str().unwrap().contains(":/") && !sil["kanit"]["dosya"].as_str().unwrap().starts_with('/'), "göreli yol: {sil}");
    // Şablondaki encodeURIComponent(id) parametre, sorgu dizgisi yolun parçası değil
    assert_eq!(bul("client:varlikGetir")["api"], "GET /api/varliklar/:id");
    // Vue tek dosya bileşeninin <script setup> bloğu: satır numarası dosyanınki
    let vue = cagrilar.iter().find(|c| c["kanit"]["dosya"].as_str().unwrap().ends_with("Eski.vue")).unwrap_or_else(|| panic!("Vue çağrısı yok: {w:#}"));
    assert_eq!(vue["api"], "POST /api/varliklar");
    assert_eq!(vue["kanit"]["satir"], 9);
    // SvelteKit ekranı
    assert!(w["ekranlar"].as_array().unwrap().iter().any(|e| e["id"] == "screen:web//fotolar"), "{w:#}");
    assert!(w["istatistik"]["uca_baglanan"].as_u64().unwrap() >= 3);
    let _ = std::fs::remove_dir_all(&kok);
}
