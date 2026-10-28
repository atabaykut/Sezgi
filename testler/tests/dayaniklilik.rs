//! Kötü niyetli ya da bozuk depolara karşı dayanıklılık: döngüler, çok derin zincirler ve iç içe ifadeler
//! Sezgi'yi çökertmemeli. Çözülemeyen şey "belirsiz" kalır, süreç düzgün biter.
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// Geçici bir NestJS uygulaması kurar: verilen dosyalar `src/` altına yazılır
fn uygulama(ad: &str, dosyalar: &[(&str, String)]) -> PathBuf {
    let t = std::env::temp_dir().join(format!("sezgi-dayanik-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    std::fs::create_dir_all(t.join("src")).unwrap();
    std::fs::write(t.join("package.json"), r#"{ "name": "dayanik", "dependencies": { "@nestjs/core": "10" } }"#).unwrap();
    for (yol, metin) in dosyalar {
        let p = t.join("src").join(yol);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, metin).unwrap();
    }
    t
}

/// `sezgi init` çalışır; çökmeden bitmeli ve haritayı yazmalı
fn haritala(t: &PathBuf) -> Value {
    let cikti = t.join("harita");
    let d = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", t.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(d.status.success(), "sezgi çöktü ya da hata verdi: {:?}\n{}", d.status, String::from_utf8_lossy(&d.stderr));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(t);
    v
}

const KONTROLCU: &str = "import { Body, Controller, Post } from '@nestjs/common';\nimport { Girdi } from './girdi';\n@Controller('a')\nexport class AController {\n  @Post() yaz(@Body() dto: Girdi) {}\n}\n";

fn govde_semasi(v: &Value) -> &Value {
    &v["endpointler"][0]["govde"][0]["sema"]
}

#[test]
fn takma_ad_dongusu_dogrudan() {
    let t = uygulama("dongu1", &[("a.controller.ts", KONTROLCU.into()), ("girdi.ts", "export { Girdi as Oteki, Oteki as Girdi };\n".into())]);
    let v = haritala(&t);
    assert_eq!(govde_semasi(&v)["bulunamadi"], true, "döngüdeki ad çözülmüş gibi gösterilmemeli");
}

#[test]
fn takma_ad_dongusu_uc_adim() {
    let t = uygulama(
        "dongu3",
        &[
            ("a.controller.ts", KONTROLCU.into()),
            ("girdi.ts", "export { Iki as Girdi };\n".into()),
            ("iki.ts", "export { Uc as Iki };\n".into()),
            ("uc.ts", "export { Girdi as Uc };\n".into()),
        ],
    );
    let v = haritala(&t);
    assert_eq!(govde_semasi(&v)["bulunamadi"], true);
}

#[test]
fn kendine_atif_ve_karsilikli_kalitim() {
    let t = uygulama(
        "kendine",
        &[("a.controller.ts", KONTROLCU.into()), ("girdi.ts", "export { Girdi as Girdi };\nexport class Girdi extends Oteki {}\nexport class Oteki extends Girdi {}\n".into())],
    );
    let v = haritala(&t);
    // Döngüsel kalıtım: alanlar kesin olarak bilinemez
    assert_eq!(govde_semasi(&v)["bulunamadi"], true);
}

#[test]
fn zod_karsilikli_atif() {
    let kontrolcu = "import { Body, Controller, Post } from '@nestjs/common';\nimport { z } from 'zod';\nimport { createZodDto } from 'nestjs-zod';\nconst A = B.extend({ a: z.string() });\nconst B = A.extend({ b: z.string() });\nexport class Girdi extends createZodDto(A) {}\n@Controller('a')\nexport class AController {\n  @Post() yaz(@Body() dto: Girdi) {}\n}\n";
    let t = uygulama("zod", &[("a.controller.ts", kontrolcu.into())]);
    let v = haritala(&t);
    assert_eq!(govde_semasi(&v)["bulunamadi"], true, "döngüsel zod şeması kesin sayılmamalı");
}

#[test]
fn cok_derin_dongusuz_zincir() {
    // 300 adımlık takma ad zinciri: çökmemeli; sınırı aşarsa belirsiz kalmalı
    let mut dosyalar = vec![("a.controller.ts", KONTROLCU.to_string())];
    let mut zincir = String::from("export { T1 as Girdi };\n");
    for i in 1..300 {
        zincir.push_str(&format!("export {{ T{} as T{} }};\n", i + 1, i));
    }
    zincir.push_str("export class T300 { ad!: string; }\n");
    dosyalar.push(("girdi.ts", zincir));
    let t = uygulama("derin", &dosyalar);
    haritala(&t);
}

#[test]
fn cok_derin_ic_ice_ifade() {
    // 100 bin iç içe parantez ve sonunda bir sözdizimi hatası: özyinelemeli gezinme yığını taşırmamalı
    let derin = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
    let kontrolcu =
        format!("import {{ Controller, Get }} from '@nestjs/common';\n@Controller('a')\nexport class AController {{\n  @Get() al() {{ return this.x.y({derin}) + ; }}\n}}\n");
    let t = uygulama("ifade", &[("a.controller.ts", kontrolcu)]);
    haritala(&t);
}

#[test]
fn cok_derin_sarmalayici() {
    // sarmala(sarmala(...(z.object({}))...)): zod sarmalayıcı özyinelemesi sınırlı olmalı
    let ic = format!("{}z.object({{ a: z.string() }}){}", "sarmala(".repeat(50_000), ")".repeat(50_000));
    let kontrolcu = format!(
        "import {{ Body, Controller, Post }} from '@nestjs/common';\nimport {{ z }} from 'zod';\nconst S = {ic};\nexport class Girdi extends createZodDto(S) {{}}\n@Controller('a')\nexport class AController {{\n  @Post() yaz(@Body() dto: Girdi) {{}}\n}}\n"
    );
    let t = uygulama("sarmal", &[("a.controller.ts", kontrolcu)]);
    // Süre de sınırlı: çağrı grafiği her iç içe çağrıda ebeveyni kökten arayıp argümanın bütün metnini okuduğunda bu dosya
    // dakikalar sürüyordu (karesel). Sınır debug derlemesi ve yavaş makineler için geniş tutuldu.
    let bas = std::time::Instant::now();
    haritala(&t);
    assert!(bas.elapsed() < std::time::Duration::from_secs(120), "50.000 iç içe çağrı {:?} sürdü", bas.elapsed());
}

#[test]
fn imzasiz_kurulum_eski_acik_anahtari_birakmaz() {
    // Önce imzalı, sonra imzasız kurulum aynı çıktı klasörüne: eski benlik-keys.json kalırsa Ayna ve
    // okuyanlar imzasız kartın yanında bir anahtar görür
    let ayar = std::env::temp_dir().join(format!("sezgi-dayanik-anahtar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ayar);
    let sezgi = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.env("APPDATA", &ayar).env("XDG_CONFIG_HOME", &ayar);
        c
    };
    assert!(sezgi().args(["anahtar", "olustur", "--ad", "k"]).output().unwrap().status.success());
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
    let h = ayar.join("harita");
    assert!(sezgi().args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap(), "--imzala", "k"]).output().unwrap().status.success());
    assert!(h.join("benlik-keys.json").exists());
    assert!(sezgi().args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap()]).output().unwrap().status.success());
    let kalan = h.join("benlik-keys.json").exists();
    // Açık anahtar dosyası kopyalanamazsa (silinmiş) imzalı kurulum başarı saymamalı
    std::fs::remove_file(ayar.join("sezgi").join("anahtarlar").join("k.benlik-keys.json")).unwrap();
    let d = sezgi().args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap(), "--imzala", "k"]).output().unwrap();
    let _ = std::fs::remove_dir_all(&ayar);
    assert!(!kalan, "imzasız kurulumdan sonra eski benlik-keys.json kaldı");
    assert!(!d.status.success(), "açık anahtar kopyalanamadı ama kurulum başarılı sayıldı");
}

#[test]
fn genel_onek_test_ve_kardes_dosyalardan_alinmaz() {
    // main.ts önek tanımlamıyor; test dosyasındaki ve yorumdaki setGlobalPrefix uygulamanın öneki değildir
    let t = uygulama(
        "onek",
        &[
            (
                "main.ts",
                "import { NestFactory } from '@nestjs/core';\nimport { AppModule } from './app.module';\nNestFactory.create(AppModule);\n// app.setGlobalPrefix('yorum');\n".into(),
            ),
            (
                "app.module.ts",
                "import { Module } from '@nestjs/common';\nimport { AController } from './a.controller';\n@Module({ controllers: [AController] })\nexport class AppModule {}\n"
                    .into(),
            ),
            ("a.controller.ts", "import { Controller, Get } from '@nestjs/common';\n@Controller('a')\nexport class AController {\n  @Get() al() {}\n}\n".into()),
            ("test/e2e.ts", "export async function kur(app: any) { app.setGlobalPrefix('test-onek'); }\n".into()),
        ],
    );
    let v = haritala(&t);
    assert_eq!(v["ozet"]["genel_onek"], "", "{}", v["ozet"]);
    assert_eq!(v["endpointler"][0]["id"], "GET /a");
}

/// Klasör bağı kurar: Unix'te sembolik bağ, Windows'ta yönetici izni gerektirmeyen dizin bağlantısı (junction)
fn klasor_bagi(hedef: &std::path::Path, bag: &std::path::Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(hedef, bag).unwrap();
    #[cfg(windows)]
    {
        let d = Command::new("cmd").args(["/C", "mklink", "/J", bag.to_str().unwrap(), hedef.to_str().unwrap()]).output().unwrap();
        assert!(d.status.success(), "{}", String::from_utf8_lossy(&d.stderr));
    }
}

#[test]
fn git_ref_ara_klasor_bagiyla_depo_disina_cikmaz() {
    // .git/refs/heads depo dışındaki bir klasöre bağ: HEAD'in gösterdiği "ref" dosyası depo dışından okunmamalı
    let t = uygulama("gitref", &[("a.controller.ts", "import { Controller, Get } from '@nestjs/common';\n@Controller('a')\nexport class A {\n  @Get() al() {}\n}\n".into())]);
    let dis = t.parent().unwrap().join(format!("sezgi-dayanik-dis-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dis);
    std::fs::create_dir_all(&dis).unwrap();
    std::fs::write(dis.join("ana"), "COKGIZLIDEGER-depo-disi").unwrap();
    std::fs::create_dir_all(t.join(".git").join("refs")).unwrap();
    std::fs::write(t.join(".git").join("HEAD"), "ref: refs/heads/ana\n").unwrap();
    klasor_bagi(&dis, &t.join(".git").join("refs").join("heads"));
    let v = haritala(&t);
    let _ = std::fs::remove_dir_all(&dis);
    assert_eq!(v["ozet"]["kaynak_ref"], "unknown", "{}", v["ozet"]["kaynak_ref"]);
}
