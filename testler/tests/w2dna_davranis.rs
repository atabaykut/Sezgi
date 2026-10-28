//! W2-dna (P2.2, DNA D2): statik davranış özeti. İşlemin gövdesinden ve çağırdığı servis / bağımlılık zincirinden
//! hata senaryoları (koşul + HTTP kodu + dosya:satır), nesne düzeyi sahiplik denetimi, davranışı ikiye bölen koşullar,
//! ORM şemasındaki kaskadla dolaylı silinen tablolar ve geri alınabilirlik; `tani`'de gösterilir, önizlemede
//! "kontrol edilemedi" listesine girer, kartın `x-dna` alanına taşınır.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-w2dna-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Kapının davranış özeti (`ozet.dna["dosya:satır"]`)
fn dna(harita: &Path, id: &str) -> Value {
    let h = json_oku(&harita.join("kapilar.json"));
    let k = h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).unwrap_or_else(|| panic!("{id} yok"));
    let yer = format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]);
    h["ozet"]["dna"].get(&yer).cloned().unwrap_or_else(|| panic!("{id} ({yer}) için dna yok: {:#}", h["ozet"]["dna"]))
}

/// Kartta kapının `x-dna`'sı
fn kart_dna(harita: &Path, yontem: &str, yol: &str) -> Value {
    let k = json_oku(&harita.join("benlik.json"));
    let op =
        k["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == yontem && o["entry"]["path"] == yol).unwrap_or_else(|| panic!("kartta {yontem} {yol} yok"));
    op["x-dna"].clone()
}

fn arac(harita: &Path, ad: &str, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn hata<'a>(d: &'a Value, durum: u64, dosya: &str) -> &'a Value {
    d["hatalar"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["durum"] == durum && h["kanit"].as_str().unwrap_or("").contains(dosya))
        .unwrap_or_else(|| panic!("{durum} ({dosya}) hatası yok: {d:#}"))
}

#[test]
fn fastapi_kaskad_sahiplik_ve_red_kosullari() {
    let h = haritala("w2dna_fastapi");
    // Altın: DELETE /users/me yalnızca User'ı siler görünür; Item kaskadla dolaylı silinir
    let me = dna(&h, "DELETE /api/v1/users/me");
    let dolayli = me["dolayli_yazar"].as_array().unwrap_or_else(|| panic!("dolaylı yazar yok: {me:#}"));
    assert!(dolayli.iter().any(|d| d["tablo"] == "Item" && d["kanit"].as_str().unwrap().contains("models.py") && d["guven"] == "inferred"), "{me:#}");
    let r = hata(&me, 403, "users.py");
    assert_eq!(r["kosul"], "current_user.is_superuser", "{r}");
    assert_eq!(r["guven"], "certain", "işlem gövdesindeki desen: {r}");
    assert_eq!(me["geri_alinabilir"], "hayir");
    // DELETE /items/:id: 404 (kayıt yok) ve 403 (sahiplik); bağımlılıktaki 404 ayrı işaretli
    let it = dna(&h, "DELETE /api/v1/items/:id");
    let yok = hata(&it, 404, "routes/items.py");
    assert_eq!(yok["kosul"], "not item", "{yok}");
    let sahip = hata(&it, 403, "routes/items.py");
    assert_eq!(sahip["sahiplik"], true, "{sahip}");
    assert!(sahip["kosul"].as_str().unwrap().contains("item.owner_id != current_user.id"), "{sahip}");
    assert!(it["sahiplik_denetimi"].as_array().unwrap().iter().any(|s| s["kanit"].as_str().unwrap().contains("items.py")), "{it:#}");
    let bag = hata(&it, 404, "deps.py");
    assert_eq!(bag["kaynak"], "bagimlilik", "{bag}");
    assert_eq!(bag["guven"], "inferred", "{bag}");
    assert!(it.get("dolayli_yazar").is_none(), "Item'ın kaskad çocuğu yok: {it:#}");
    // Okuma işleminde geri alma sorulmaz; status.HTTP_404_NOT_FOUND koda çözülür
    let oku = dna(&h, "GET /api/v1/items/:id");
    assert!(oku.get("geri_alinabilir").is_none(), "{oku:#}");
    hata(&oku, 404, "routes/items.py");
    // Kart: hata senaryoları, dolaylı yazma, geri alınamazlık bilinmiyor
    let k = kart_dna(&h, "DELETE", "/api/v1/items/:id");
    assert!(k["failure_modes"].as_array().unwrap().iter().any(|f| f["result"] == "HTTP 403 (HTTPException)" && f["condition"].as_str().unwrap().contains("owner_id")), "{k:#}");
    assert_eq!(k["coverage"]["failure_modes"], "open");
    let k = kart_dna(&h, "DELETE", "/api/v1/users/me");
    let dolayli = k["data_effect"]["indirect"].as_array().unwrap_or_else(|| panic!("data_effect.indirect yok: {k:#}"));
    assert!(
        dolayli
            .iter()
            .any(|d| d["table"] == "Item" && d["stamp"] == "inferred" && d["sources"][0]["file"].as_str().unwrap().ends_with("models.py") && d["sources"][0]["kind"] == "schema"),
        "{k:#}"
    );
    assert!(!k["data_effect"]["writes"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("indirect")), "geçici yazım kalktı: {k:#}");
    assert_eq!(k["reversibility"]["status"], "irreversible");
    // Kanıt deposu: hata senaryosu (VALIDATES), sahiplik ve dolaylı yazma (MAY_AFFECT, hiçbir zaman certain değil)
    let kn = arac(&h, "kanit", json!({ "kapi": "DELETE /api/v1/users/me" }));
    assert!(kn.contains("MAY_AFFECT") && kn.contains("db:Item") || kn.contains("MAY_AFFECT") && kn.contains("Item"), "{kn}");
    assert!(kn.contains("models.py:"), "dolaylı yazmanın kanıtı şema satırı: {kn}");
    assert!(kn.contains("VALIDATES") && kn.contains("routes/users.py:16"), "{kn}");
    let depo = json_oku(&h.join("kanit.json"));
    let ma =
        depo["onermeler"].as_array().unwrap().iter().find(|o| o["yuklem"] == "MAY_AFFECT" && o["hedef"] == "db:Item").unwrap_or_else(|| panic!("MAY_AFFECT db:Item önermesi yok"));
    assert_eq!(ma["hesap"]["guven"], "inferred", "{ma}");
    assert_eq!(ma["hesap"]["kapsam"], "acik", "{ma}");
    // etki: DELETE /users/me -> Item
    let e = arac(&h, "etki", json!({ "kapi": "DELETE /api/v1/users/me" }));
    assert!(e.contains("Item") && e.contains("may_affect"), "{e}");
    let kn = arac(&h, "kanit", json!({ "kapi": "DELETE /api/v1/items/:id" }));
    assert!(kn.contains("SAHIPLIK") && kn.contains("routes/items.py:"), "{kn}");
    // Önizleme: sahiplik ve kayıt yok koşulları "kontrol edilemedi"; sahiplik önce
    let m = arac(&h, "onizle", json!({ "kapi": "DELETE /api/v1/items/:id", "oturum": true, "parametreler": { "id": "x" } }));
    assert!(m.contains("BİLİNEN KURALLARI GEÇİYOR"), "{m}");
    let sorular: Vec<&str> = m.lines().filter(|l| l.starts_with("  ? ")).collect();
    assert!(sorular.len() <= 5, "{m}");
    let i_sahip = sorular.iter().position(|l| l.contains("yetki (nesne düzeyi)") && l.contains("items.py:")).unwrap_or_else(|| panic!("sahiplik maddesi yok:\n{m}"));
    let i_kural = sorular.iter().position(|l| l.contains("iş kuralı") && l.contains("→ 404")).unwrap_or_else(|| panic!("404 maddesi yok:\n{m}"));
    assert!(i_sahip < i_kural, "sahiplik önce gelmeli:\n{m}");
    // tani
    let t = arac(&h, "tani", json!({ "kapi": "DELETE /api/v1/users/me" }));
    assert!(t.contains("dolaylı yazar: Item") && t.contains("red 403"), "{t}");
    let t = arac(&h, "tani", json!({ "kapi": "DELETE /api/v1/items/:id" }));
    assert!(t.contains("nesne düzeyi sahiplik denetimi") && t.contains("geri alma: hayır"), "{t}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn nest_servis_zinciri_iki_yol_ve_geri_alma() {
    let h = haritala("w2dna_nest");
    // Toplu silme: force davranışı ikiye böler; erişim yardımcısındaki 400 servis zincirinden (üst sınıf + üst düzey fonksiyon)
    let t = dna(&h, "DELETE /api/varliklar");
    assert!(
        t["dallar"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["kosul"] == "force" && d["evet"] == "VarlikDurumu.Silindi" && d["hayir"] == "VarlikDurumu.Copte" && d["baglam"] == "durum"),
        "{t:#}"
    );
    let r = hata(&t, 400, "ortak/erisim.ts");
    assert_eq!(r["istisna"], "BadRequestException");
    assert_eq!(r["kaynak"], "servis");
    assert_eq!(r["guven"], "inferred");
    assert!(r["kosul"].as_str().unwrap().starts_with("!ids.every"), "{r}");
    assert_eq!(t["geri_alinabilir"], "kismen", "deletedAt yumuşak silme, force ile kalıcı: {t:#}");
    assert!(t["geri_alma_gerekce"].as_str().unwrap().contains("varlik.service.ts"), "{t:#}");
    // Tek silme: 404, sahiplik 403, HttpStatus.CONFLICT -> 409; kaskad (Prisma onDelete: Cascade); geri yükleme ucu
    let s = dna(&h, "DELETE /api/varliklar/:id");
    assert_eq!(hata(&s, 404, "varlik.service.ts")["kosul"], "!varlik");
    let sahip = hata(&s, 403, "varlik.service.ts");
    assert_eq!(sahip["sahiplik"], true, "{sahip}");
    assert_eq!(sahip["istisna"], "ForbiddenException");
    hata(&s, 409, "varlik.service.ts");
    assert!(s["dolayli_yazar"].as_array().unwrap().iter().any(|d| d["tablo"] == "Etiket" && d["kanit"].as_str().unwrap().contains("schema.prisma")), "{s:#}");
    assert_eq!(s["geri_alinabilir"], "kismen");
    assert!(s["geri_alma_gerekce"].as_str().unwrap().contains("POST /api/cop/geri-yukle/varliklar"), "{s:#}");
    // Kart
    let k = kart_dna(&h, "DELETE", "/api/varliklar");
    assert_eq!(k["reversibility"]["status"], "partial");
    assert_eq!(k["reversibility"]["stamp"], "inferred");
    assert!(k["failure_modes"].as_array().unwrap().iter().any(|f| f["result"] == "HTTP 400 (BadRequestException)" && f["stamp"] == "inferred"), "{k:#}");
    let k = kart_dna(&h, "DELETE", "/api/varliklar/:id");
    assert!(k["data_effect"]["indirect"].as_array().unwrap().iter().any(|d| d["table"] == "Etiket" && d["stamp"] == "inferred"), "{k:#}");
    // Kart: iki yollu koşul `branches`
    let k = kart_dna(&h, "DELETE", "/api/varliklar");
    assert!(
        k["branches"].as_array().unwrap_or_else(|| panic!("branches yok: {k:#}")).iter().any(|b| b["condition"] == "force"
            && b["when_true"] == "durum: VarlikDurumu.Silindi"
            && b["when_false"] == "durum: VarlikDurumu.Copte"
            && b["sources"][0]["file"].as_str().unwrap().ends_with("varlik.service.ts")),
        "{k:#}"
    );
    // tani: iki yol görünür
    let m = arac(&h, "tani", json!({ "kapi": "DELETE /api/varliklar" }));
    assert!(m.contains("iki yol: `force` ? VarlikDurumu.Silindi : VarlikDurumu.Copte"), "{m}");
    assert!(m.contains("geri alma: kısmen"), "{m}");
    // Önizleme: GEÇER denmez, servis içi kural listelenir
    let m = arac(&h, "onizle", json!({ "kapi": "DELETE /api/varliklar/:id", "oturum": true, "parametreler": { "id": "1" } }));
    assert!(!m.contains("GEÇER (bilinen bütün kurallar)"), "{m}");
    assert!(m.contains("yetki (nesne düzeyi)") && m.contains("varlik.service.ts:"), "{m}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn express_isleyici_ve_servis_red_kosullari_kaskad() {
    let h = haritala("w2dna_express");
    let d = dna(&h, "DELETE /api/makaleler/:id");
    let istek = hata(&d, 400, "makale.router.js");
    assert_eq!(istek["kosul"], "!req.params.id");
    assert_eq!(istek["guven"], "certain");
    let yok = hata(&d, 404, "makale.service.js");
    assert_eq!(yok["guven"], "inferred");
    let sahip = hata(&d, 403, "makale.service.js");
    assert_eq!(sahip["sahiplik"], true, "{sahip}");
    assert!(d["dolayli_yazar"].as_array().unwrap().iter().any(|x| x["tablo"] == "Yorum"), "zincirde prisma.makale.delete + onDelete: Cascade: {d:#}");
    assert_eq!(d["geri_alinabilir"], "hayir");
    let _ = std::fs::remove_dir_all(&h);
}
