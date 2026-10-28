//! W3-yanıt: yanıt şemaları ve ön yüz ekran geçişleri.
//! - FastAPI `response_model=` / dönüş tipi, NestJS `@ApiOkResponse({ type })` / metodun dönüş tipi, Express `res.json({...})`
//!   (yalnızca yorum) → kapının `yanit` şeması; kanıt deposunda `YANIT_SEMASI` (açık beyan certain, çıkarım inferred),
//!   kartta `operation.x-response`, OpenAPI'de `responses.<durum>.content`.
//! - Gözlenen yanıt alanları bu şemayla karşılaştırılır: uyumluysa destek ve gözlenen `YANIT` kapalı kapsam; değilse çelişki.
//! - Ön yüz: `navigate`, `redirect`, `<Link to>`, yol listesi + değişken bağlantı → `screen -[NAVIGATES]-> screen` (inferred);
//!   yolculuk girişten işlemin ekranına ara ekranlarla.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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

fn degistir(f: &Path, eski: &str, yeni: &str) {
    let m = std::fs::read_to_string(f).unwrap().replace("\r\n", "\n");
    assert!(m.contains(eski), "{}: {eski}", f.display());
    std::fs::write(f, m.replacen(eski, yeni, 1)).unwrap();
}

/// Örneği kopyalar; `uygulama` alt klasörse (backend) init orada
fn proje(ev: &Ev, ornek_ad: &str, ad: &str, uygulama: &str, hazirla: impl Fn(&Path)) -> Proje {
    let kok = ev.kok.join(ad);
    kopyala(&ornek(ornek_ad), &kok);
    hazirla(&kok);
    let p = Proje { klasor: if uygulama.is_empty() { kok } else { kok.join(uygulama) }, ev: ev.kok.join("ev") };
    p.ok(&["init", "."]);
    p
}

fn oku(p: &Proje, ad: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join(ad)).unwrap_or_else(|e| panic!("{ad}: {e}"))).unwrap()
}

fn kapi<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).unwrap_or_else(|| panic!("kapı yok: {id}"))
}

fn onerme<'a>(k: &'a Value, varlik: &str, yuklem: &str, hedef: Option<&str>) -> Option<&'a Value> {
    k["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"] == varlik && o["yuklem"] == yuklem && o["hedef"].as_str() == hedef)
}

fn destek_turleri(k: &Value, o: &Value) -> Vec<String> {
    o["hesap"]["destek"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| k["kanitlar"].as_array().unwrap().iter().find(|x| x["id"] == *i).map(|x| x["kaynak_turu"].as_str().unwrap().to_string()).unwrap_or_default())
        .collect()
}

fn alan_adlari(s: &Value) -> Vec<String> {
    s["alanlar"].as_object().unwrap().keys().cloned().collect()
}

fn islem<'a>(kart: &'a Value, yontem: &str, yol: &str) -> &'a Value {
    kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == yontem && o["entry"]["path"] == yol).unwrap_or_else(|| panic!("işlem yok: {yontem} {yol}"))
}

fn sablon(ev: &Ev, ad: &str) -> Proje {
    proje(ev, "w2g_fastapi_template", ad, "backend", |_| {})
}

#[test]
fn w3y_fastapi_yanit_semasi_harita_kanit_kart_openapi() {
    let ev = Ev::yeni("w3y-fastapi");
    let p = sablon(&ev, "t");
    let h = oku(&p, "kapilar.json");
    // response_model=ItemPublic (açık beyan)
    let y = &kapi(&h, "POST /api/v1/items/")["yanit"];
    assert_eq!((y["kaynak"].as_str(), y["kesin"].as_bool(), y["sema"]["dto"].as_str()), (Some("response_model"), Some(true), Some("ItemPublic")), "{y}");
    assert_eq!(alan_adlari(&y["sema"]), vec!["created_at", "description", "id", "owner_id", "title"]);
    // Liste modeli (ItemsPublic: data, count) ve dönüş tipi bildirimi (-> Message)
    assert_eq!(alan_adlari(&kapi(&h, "GET /api/v1/items/")["yanit"]["sema"]), vec!["count", "data"]);
    let sil = &kapi(&h, "DELETE /api/v1/items/:id")["yanit"];
    assert_eq!((sil["kaynak"].as_str(), sil["sema"]["dto"].as_str()), (Some("donus_tipi"), Some("Message")), "{sil}");
    // Yanıtı olmayan uç (-> Any ya da bildirimsiz) şemasız kalır: uydurulmaz
    assert!(kapi(&h, "GET /api/v1/utils/health-check/").get("yanit").is_none());
    // Kanıt: certain, kapalı kapsam
    let k = oku(&p, "kanit.json");
    let o = onerme(&k, "api:POST /api/v1/items/", "YANIT_SEMASI", Some("ItemPublic")).unwrap();
    assert_eq!((o["hesap"]["guven"].as_str(), o["hesap"]["kapsam"].as_str()), (Some("certain"), Some("kapali")), "{o}");
    // Kart: operation.x-response
    let kart = p.kart();
    let xr = &islem(&kart, "POST", "/api/v1/items/")["x-response"];
    assert_eq!((xr["type"].as_str(), xr["stamp"].as_str(), xr["basis"].as_str()), (Some("ItemPublic"), Some("certain"), Some("response_model")), "{xr}");
    assert_eq!(xr["fields"]["title"]["type"], "string");
    assert!(xr["sources"][0]["file"].as_str().unwrap().ends_with("models.py"), "{xr}");
    assert!(kart["x-basis"].as_object().unwrap().keys().any(|f| f.ends_with("models.py")), "yanıt şemasının dosyası tazelik dayanağında");
    // OpenAPI: responses.200 → components.schemas.ItemPublic
    let oa = oku(&p, "openapi.json");
    let r = &oa["paths"]["/api/v1/items/"]["post"]["responses"]["200"];
    assert_eq!(r["content"]["application/json"]["schema"]["$ref"], "#/components/schemas/ItemPublic", "{r}");
    let s = &oa["components"]["schemas"]["ItemPublic"];
    assert!(s["properties"]["owner_id"].is_object() && s["required"].as_array().unwrap().contains(&json!("id")), "{s}");
    let liste = &oa["paths"]["/api/v1/items/"]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
    assert_eq!(liste["$ref"], "#/components/schemas/ItemsPublic", "{liste}");
}

/// Elle yazılmış küçük HAR
fn har_yaz(p: &Path, istekler: &[(&str, &str, Option<Value>, u16, Value)]) {
    let girdiler: Vec<Value> = istekler
        .iter()
        .enumerate()
        .map(|(i, (y, yol, govde, durum, yanit))| {
            let mut istek = json!({ "method": y, "url": format!("http://localhost:8000{yol}"), "headers": [] });
            if let Some(g) = govde {
                istek["postData"] = json!({ "mimeType": "application/json", "text": g.to_string() });
            }
            json!({ "startedDateTime": format!("2026-10-21T10:00:0{i}.000Z"), "request": istek,
                    "response": { "status": durum, "headers": [], "content": { "mimeType": "application/json", "text": yanit.to_string() } } })
        })
        .collect();
    std::fs::write(p, json!({ "log": { "version": "1.2", "entries": girdiler } }).to_string()).unwrap();
}

#[test]
fn w3y_gozlenen_yanit_semayla_karsilastirilir() {
    let ev = Ev::yeni("w3y-gozlem");
    // Uyumlu: gerçek kayıt (Test 1 yeniden üretimi)
    let p = sablon(&ev, "a");
    p.ok(&["gozlem", "al", ornek("w2g_har").join("oturum.har").to_str().unwrap()]);
    let k = oku(&p, "kanit.json");
    let o = onerme(&k, "api:POST /api/v1/items/", "YANIT_SEMASI", Some("ItemPublic")).unwrap();
    let t = destek_turleri(&k, o);
    assert!(t.contains(&"SOURCE_CODE".to_string()) && t.contains(&"NETWORK_CAPTURE".to_string()), "{t:?}");
    assert!(o["hesap"]["celiski"].as_array().unwrap().is_empty(), "{o}");
    assert_eq!(o["hesap"]["guven"], "certain");
    // Şemalı ve uyumlu uçta gözlenen yanıt biçimi kapalı kapsam
    let y = onerme(&k, "api:POST /api/v1/items/", "YANIT", Some("200")).unwrap();
    assert_eq!((y["hesap"]["guven"].as_str(), y["hesap"]["kapsam"].as_str()), (Some("observed"), Some("kapali")), "{y}");
    // Liste ucu da uyumlu (ItemsPublic: data, count)
    assert!(onerme(&k, "api:GET /api/v1/items/", "YANIT_SEMASI", Some("ItemsPublic")).unwrap()["hesap"]["celiski"].as_array().unwrap().is_empty());
    // Uyuşmaz: koddaki şemada olmayan alan ve eksik zorunlu alan
    let q = sablon(&ev, "b");
    let har = ev.kok.join("celiski.har");
    har_yaz(&har, &[("POST", "/api/v1/items/", Some(json!({ "title": "x" })), 200, json!({ "id": 1, "title": "x", "gizli_alan": true }))]);
    q.ok(&["gozlem", "al", har.to_str().unwrap()]);
    let k = oku(&q, "kanit.json");
    let o = onerme(&k, "api:POST /api/v1/items/", "YANIT_SEMASI", Some("ItemPublic")).unwrap();
    assert!(!o["hesap"]["celiski"].as_array().unwrap().is_empty(), "{o}");
    assert_eq!(o["hesap"]["guven"], "inferred", "çelişkide en fazla inferred");
    let y = onerme(&k, "api:POST /api/v1/items/", "YANIT", Some("200")).unwrap();
    assert_eq!(y["hesap"]["kapsam"], "acik", "uyuşmayan gözlem kapalı kapsam almaz");
    let (s, _) = q.mcp("").arac("kanit", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(s.contains("yanıtta kodda olmayan alan: gizli_alan") && s.contains("yanıtta zorunlu `owner_id` yok"), "{s}");
}

#[test]
fn w3y_nestjs_api_response_ve_donus_tipi() {
    let ev = Ev::yeni("w3y-nest");
    let p = proje(&ev, "nestjs-temel", "n", "", |k| {
        let dto = k.join("src/kitap/dto/kitap.dto.ts");
        let m = std::fs::read_to_string(&dto).unwrap().replace("\r\n", "\n");
        std::fs::write(&dto, format!("{m}\nexport class KitapDto {{\n  @IsString() id!: string;\n  @IsString() baslik!: string;\n  @IsOptional() @IsInt() sayfa?: number;\n}}\n"))
            .unwrap();
        let c = k.join("src/kitap/kitap.controller.ts");
        degistir(&c, "import { KitapOlusturDto, KitapGuncelleDto } from './dto/kitap.dto';", "import { KitapOlusturDto, KitapGuncelleDto, KitapDto } from './dto/kitap.dto';");
        degistir(
            &c,
            "listele(@Query('ara') ara?: string, @Query('sayfa') sayfa?: number, @Query('etiket') etiket?: string[]) {",
            "listele(@Query('ara') ara?: string, @Query('sayfa') sayfa?: number, @Query('etiket') etiket?: string[]): Promise<KitapDto[]> {",
        );
        degistir(
            &c,
            "  @Post()\n  @ApiOperation({ summary: 'Kitap oluştur' })",
            "  @Post()\n  @ApiCreatedResponse({ type: KitapDto })\n  @ApiOperation({ summary: 'Kitap oluştur' })",
        );
        degistir(&c, "  @Patch(':id')\n", "  @Patch(':id')\n  @ApiResponse({ status: 404, type: KitapDto })\n");
    });
    let h = oku(&p, "kapilar.json");
    let id = |y: &str, yol: &str| h["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == y && k["yol"].as_str().unwrap().ends_with(yol)).unwrap().clone();
    let liste = id("GET", "/kitaplar");
    assert_eq!(
        (liste["yanit"]["kaynak"].as_str(), liste["yanit"]["kesin"].as_bool(), liste["yanit"]["sema"]["dizi"].as_bool(), liste["yanit"]["durum"].as_u64()),
        (Some("donus_tipi"), Some(false), Some(true), Some(200)),
        "{}",
        liste["yanit"]
    );
    assert_eq!(alan_adlari(&liste["yanit"]["sema"]), vec!["baslik", "id", "sayfa"]);
    let olustur = id("POST", "/kitaplar");
    assert_eq!(
        (olustur["yanit"]["kaynak"].as_str(), olustur["yanit"]["kesin"].as_bool(), olustur["yanit"]["durum"].as_u64()),
        (Some("ApiResponse"), Some(true), Some(201)),
        "{}",
        olustur["yanit"]
    );
    // Yalnızca hata yanıtı tanımlı ve dönüş tipi yok: şema yok
    assert!(id("PATCH", "/kitaplar/:id").get("yanit").is_none());
    // Kart: dönüş tipi çıkarım (inferred), açık beyan certain
    let kart = p.kart();
    let liste_op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "GET" && o["entry"]["path"].as_str().unwrap().ends_with("/kitaplar")).unwrap();
    assert_eq!((liste_op["x-response"]["stamp"].as_str(), liste_op["x-response"]["array"].as_bool()), (Some("inferred"), Some(true)));
    let k = oku(&p, "kanit.json");
    let o = k["onermeler"].as_array().unwrap().iter().find(|o| o["yuklem"] == "YANIT_SEMASI" && o["varlik"].as_str().unwrap().starts_with("api:GET /api/v1/kitaplar")).unwrap();
    assert_eq!((o["hesap"]["guven"].as_str(), o["hesap"]["kapsam"].as_str()), (Some("inferred"), Some("acik")));
    let oa = oku(&p, "openapi.json");
    let post = oa["paths"].as_object().unwrap().iter().find(|(y, _)| y.ends_with("/kitaplar")).unwrap().1["post"].clone();
    assert_eq!(post["responses"]["201"]["content"]["application/json"]["schema"]["$ref"], "#/components/schemas/KitapDto", "{post}");
}

#[test]
fn w3y_express_res_json_yalnizca_yorum_ve_react_router_gecisleri() {
    let ev = Ev::yeni("w3y-express");
    let p = proje(&ev, "s4_rtk", "r", "", |k| {
        let on = k.join("frontend/src");
        degistir(
            &on.join("index.js"),
            "      <Route path='/admin/productlist' element={<ProductListScreen />} />",
            "      <Route path='/admin/productlist' element={<ProductListScreen />} />\n      <Route path='/login' element={<LoginScreen />} />",
        );
        degistir(
            &on.join("index.js"),
            "import ProductListScreen from './screens/admin/ProductListScreen';",
            "import ProductListScreen from './screens/admin/ProductListScreen';\nimport LoginScreen from './screens/LoginScreen';",
        );
        std::fs::write(
            on.join("screens/LoginScreen.jsx"),
            "import { Link } from 'react-router-dom';\nconst LoginScreen = () => <Link to='/admin/productlist?x=1'>Ürünler</Link>;\nexport default LoginScreen;\n",
        )
        .unwrap();
    });
    let h = oku(&p, "kapilar.json");
    let sil = h["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == "DELETE").unwrap();
    assert_eq!((sil["yanit"]["kaynak"].as_str(), sil["yanit"]["kesin"].as_bool()), (Some("res.json"), Some(false)), "{}", sil["yanit"]);
    assert_eq!(alan_adlari(&sil["yanit"]["sema"]), vec!["message"]);
    // Boş nesne yanıtı alan vermez: şema yok
    assert!(h["endpointler"].as_array().unwrap().iter().filter(|k| k["yontem"] == "GET").all(|k| k.get("yanit").is_none()));
    let k = oku(&p, "kanit.json");
    let o = k["onermeler"].as_array().unwrap().iter().find(|o| o["yuklem"] == "YANIT_SEMASI").unwrap();
    assert_eq!(o["hesap"]["guven"], "inferred");
    // Ekran geçişleri: navigate('/login') (Header, "/" ekranında) ve <Link to> (giriş ekranı)
    let w = oku(&p, "web-baglari.json");
    let gecis: Vec<(String, String)> = w["baglar"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["iliski"] == "NAVIGATES")
        .map(|b| (b["kaynak"].as_str().unwrap().to_string(), b["hedef"].as_str().unwrap().to_string()))
        .collect();
    assert!(gecis.contains(&("screen:web//".into(), "screen:web//login".into())), "{gecis:?}");
    assert!(gecis.contains(&("screen:web//login".into(), "screen:web//admin/productlist".into())), "{gecis:?}");
    assert!(w["baglar"].as_array().unwrap().iter().filter(|b| b["iliski"] == "NAVIGATES").all(|b| b["guven"] == "inferred"));
    let g = oku(&p, "graf.json");
    assert!(g["kenarlar"].as_array().unwrap().iter().any(|e| e["iliski"] == "NAVIGATES" && e["guven"] == "inferred"));
}

#[test]
fn w3y_yolculuk_ara_ekranlarla() {
    let ev = Ev::yeni("w3y-yolculuk");
    let p = sablon(&ev, "t");
    let w = oku(&p, "web-baglari.json");
    let gecis = |a: &str, b: &str| w["baglar"].as_array().unwrap().iter().any(|x| x["iliski"] == "NAVIGATES" && x["kaynak"] == a && x["hedef"] == b);
    // Giriş ekranından "/" (logo bağlantısı, giriş sonrası yönlendirme) ve "/"dan kenar çubuğu yol listesiyle /items
    assert!(gecis("screen:web//login", "screen:web//"), "{}", w["baglar"]);
    assert!(gecis("screen:web//", "screen:web//items"), "{}", w["baglar"]);
    assert!(gecis("screen:web//items", "screen:web//settings"), "düzen rotasının içe aktardığı kenar çubuğu alt ekranlarda");
    let k = oku(&p, "kanit.json");
    let j = k["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["yuklem"] == "VAR" && o["deger"]["ad"] == "/login › Log In → / → /items › Save → POST /api/v1/items/")
        .unwrap_or_else(|| panic!("yolculuk yok"));
    assert_eq!(j["deger"]["adimlar"].as_array().unwrap().len(), 5);
    assert_eq!(j["deger"]["adimlar"][2], "screen:web//");
    let kart = p.kart();
    let kj = kart["x-journeys"].as_array().unwrap().iter().find(|x| x["id"] == j["varlik"]).unwrap();
    assert_eq!(kj["steps"].as_array().unwrap().iter().map(|s| s["kind"].as_str().unwrap()).collect::<Vec<_>>(), vec!["screen", "ui", "screen", "screen", "ui"]);
    assert!(kj["steps"].as_array().unwrap().iter().all(|s| s["stamp"] == "inferred"));
}
