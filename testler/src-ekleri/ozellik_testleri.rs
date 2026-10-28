//! Özellik tabanlı testler (proptest). Rastgele NestJS kaynakları (bilinen kalıp parçaları ve rastgele gürültü)
//! ve rastgele kapı durumları üretilir; şu değişmezler sınanır:
//! 1. Herhangi bir alt bileşen belirsizse (girdi, alan, giriş, rol) üst sonuç (işlem damgası) asla kesin değildir.
//! 2. Hiçbir girdi çökmeye (paniğe) yol açmaz.
//! 3. Aynı girdi her zaman aynı sonucu verir.
use crate::adaptor::{Alan, Girdi, Kapi, Kurallar, Motor, Sema};
use proptest::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Sanal bir uygulamayı (dosya adı, içerik) baştan sona işler: ayrıştırma, adaptör, Benlik Kartı. Diske dokunmaz.
fn isle(dosyalar: &[(String, String)]) -> (Value, Value, Vec<Kapi>) {
    let kok = PathBuf::from("/sanal");
    let parcalar: Vec<crate::model::Dosya> =
        dosyalar.iter().filter_map(|(ad, metin)| crate::model::ayristir_metin(&kok.join("src").join(ad), metin.clone().into_bytes())).collect();
    let kurallar: Kurallar = toml::from_str(crate::NESTJS).unwrap();
    let motor = Motor::yeni(&kurallar, &kok, &parcalar).veri_semalari(&["model Kitap {\n id String @id\n}\n".to_string()]);
    let (kapilar, bilgi) = motor.kapilar();
    let kart = crate::benlik::kart("sanal", "1.0.0", "unknown", "nestjs@test", &kapilar, "2026-01-01T00:00:00Z");
    (serde_json::json!({ "bilgi": bilgi, "kapilar": kapilar }), kart, kapilar)
}

/// Kartta "certain" işlemin dayandığı her olgu da kesin olmalı (SPEC 3)
fn kart_degismezi(kapilar: &[Kapi], kart: &Value) -> Result<(), String> {
    let ana: Vec<&Kapi> = kapilar.iter().filter(|k| k.ana).collect();
    for (op, k) in kart["operations"].as_array().unwrap().iter().zip(ana) {
        let girdiler = op["inputs"].as_object().unwrap();
        let belirsiz_girdi = girdiler.values().any(|g| g["stamp"] != "certain");
        if op["stamp"] == "certain" && (belirsiz_girdi || k.giris_kaniti != "kesin" || k.roller_kaniti.is_some()) {
            return Err(format!("işlem {} certain ama dayandığı bir olgu kesin değil: {op}", op["id"]));
        }
        if girdiler.values().any(|g| g["stamp"] == "unknown") && op["stamp"] != "unknown" {
            return Err(format!("işlem {} bilinmeyen girdiye rağmen {}", op["id"], op["stamp"]));
        }
        for g in k.govde.iter().chain(&k.sorgu) {
            if let Some(s) = &g.sema {
                for (ad, a) in &s.alanlar {
                    if a.belirsiz && girdiler.get(ad).is_some_and(|x| x["stamp"] == "certain") {
                        return Err(format!("{ad} belirsiz ama kartta certain"));
                    }
                }
            }
        }
    }
    Ok(())
}

// ---- Rastgele NestJS kaynakları ----

const ALAN_DEKORATORLERI: &[&str] = &[
    "@IsString()",
    "@IsOptional()",
    "@IsInt({ each: true })",
    "@ValidateIf((o) => o.x)",
    "@ApiProperty()",
    "@SecimliMetin()",
    "@Kosullu({ bos: true })",
    "@Bilinmeyen()",
    "@IsEnum(Rol)",
    "@IsIn(['a', 'b'])",
    "@Min(1)",
    "@Matches(/^\\d+$/)",
];
const TURETMELER: &[&str] = &[
    "",
    "extends Temel",
    "extends PartialType(Temel)",
    "extends PickType(Temel, ['ad'] as const)",
    "extends OmitType(PartialType(Temel), ['id'])",
    "extends IntersectionType(Temel, Ek)",
    "extends PickType(Temel, SECILEN)",
    "extends mixin(Temel)",
    "extends Temel<T>",
    "extends createZodDto(Sema)",
    "extends Girdi",
];
const METOT_DEKORATORLERI: &[&str] = &[
    "@Public()",
    "@SkipAuth()",
    "@SetMetadata('isPublic', true)",
    "@UseGuards(JwtAuthGuard)",
    "@UseGuards(OptionalJwtGuard)",
    "@UseGuards(AuthGuard('google'))",
    "@UseGuards(AuthGuard('jwt'), RolesGuard)",
    "@Roles('yonetici')",
    "@Roles([Rol.A, 'b'])",
    "@Roles(SayiRol.A)",
    "@ApiOperation({ summary: 'x', deprecated: false })",
    "@ApiOperation({ summary: 'y', deprecated: true })",
    "@Version('2')",
];
const ROTALAR: &[&str] = &["@Get()", "@Get(':id')", "@Post()", "@Post(['a', 'b'])", "@Patch(':id')", "@Delete(':id')", "@Put(Yollar.X)"];
const PARAMETRELER: &[&str] = &["", "@Body() dto: Girdi", "@Body(new ValidationPipe()) dto: Girdi", "@Body('ad') ad: string", "@Query() q: Girdi", "@Param('id') id: string"];
const CONTROLLER_ADRESLERI: &[&str] = &["'a'", "['a', 'b']", "{ path: 'c', version: '1' }", "{ path: ['d', 'e'] }", "Yollar.X", ""];
const SAGLAYICILAR: &[&str] = &[
    "",
    "providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }]",
    "providers: [{ useClass: JwtAuthGuard, provide: APP_GUARD }]",
    "providers: [{ provide: APP_GUARD, useFactory: () => new X() }]",
    "providers: [ /* { provide: APP_GUARD, useClass: JwtAuthGuard } */ ]",
];

fn sec(liste: &'static [&'static str]) -> impl Strategy<Value = &'static str> {
    prop::sample::select(liste)
}

fn alan() -> impl Strategy<Value = String> {
    (prop::collection::vec(sec(ALAN_DEKORATORLERI), 0..3), "[a-z]{1,6}", any::<bool>())
        .prop_map(|(ds, ad, soru)| format!("  {} {ad}{}: string;", ds.join(" "), if soru { "?" } else { "!" }))
}

fn metot() -> impl Strategy<Value = String> {
    (prop::collection::vec(sec(METOT_DEKORATORLERI), 0..3), sec(ROTALAR), sec(PARAMETRELER), "[a-z]{1,6}")
        .prop_map(|(ds, rota, param, ad)| format!("  {} {rota}\n  {ad}({param}) {{ return this.servis.prisma.kitap.findMany(); }}", ds.join("\n  ")))
}

/// Rastgele bir uygulama: DTO'lar, controller, modül, guard, işaret tanımları ve isteğe bağlı rastgele gürültü
fn uygulama() -> impl Strategy<Value = Vec<(String, String)>> {
    (
        prop::collection::vec(alan(), 0..5),
        sec(TURETMELER),
        prop::collection::vec(metot(), 1..4),
        sec(CONTROLLER_ADRESLERI),
        prop::collection::vec(sec(METOT_DEKORATORLERI), 0..2),
        sec(SAGLAYICILAR),
        prop::option::of("\\PC{0,40}"),
        any::<bool>(),
    )
        .prop_map(|(alanlar, turetme, metotlar, adres, sinif_dek, saglayici, gurultu, guard_okur)| {
            let dto = format!(
                "import {{ IsString, IsOptional, IsInt, ValidateIf, IsEnum, IsIn, Min, Matches }} from 'class-validator';\nimport {{ SecimliMetin, Kosullu }} from './dogrulama';\nexport enum Rol {{ A = 'a', B = 'b' }}\nexport enum SayiRol {{ A = 1 }}\nconst SECILEN = ['ad'] as const;\nexport class Temel {{\n  @IsString() id!: string;\n  @IsString() ad!: string;\n}}\nexport class Ek {{\n  @IsOptional() @IsString() not?: string;\n}}\nexport class Girdi {turetme} {{\n{}\n}}\n",
                alanlar.join("\n")
            );
            let dogrulama = "import { applyDecorators, SetMetadata } from '@nestjs/common';\nimport { IsOptional, IsString } from 'class-validator';\nexport const SecimliMetin = () => applyDecorators(IsOptional(), IsString());\nexport function Kosullu(s?: { bos?: boolean }) { const d = [IsString()]; if (s?.bos) { d.push(IsOptional()); } return applyDecorators(...d); }\nexport const IS_PUBLIC = 'isPublic';\nexport const Public = () => SetMetadata(IS_PUBLIC, true);\nexport const SkipAuth = () => SetMetadata(IS_PUBLIC, true);\n";
            let guard = format!(
                "import {{ AuthGuard }} from '@nestjs/passport';\nimport {{ Reflector }} from '@nestjs/core';\nexport class JwtAuthGuard extends AuthGuard('jwt') {{\n  constructor(private reflector: Reflector) {{ super(); }}\n  canActivate(c: any) {{ return this.reflector.getAllAndOverride({}, [c.getHandler()]) || super.canActivate(c); }}\n}}\nexport class OptionalJwtGuard extends AuthGuard('jwt') {{}}\n",
                if guard_okur { "IS_PUBLIC" } else { "'baska'" }
            );
            let kontrolcu = format!(
                "import {{ Body, Controller, Get, Post, Patch, Delete, Put, Param, Query, UseGuards, SetMetadata, Version }} from '@nestjs/common';\nimport {{ Public, SkipAuth }} from './dogrulama';\nimport {{ Girdi, Rol, SayiRol }} from './girdi.dto';\nexport enum Yollar {{ X = 'x' }}\n{}\n@Controller({adres})\nexport class AController {{\n  constructor(private readonly servis: any) {{}}\n{}\n}}\n{}",
                sinif_dek.join("\n"),
                metotlar.join("\n"),
                gurultu.unwrap_or_default()
            );
            let modul = format!(
                "import {{ Module }} from '@nestjs/common';\nimport {{ APP_GUARD }} from '@nestjs/core';\nimport {{ AController }} from './a.controller';\nimport {{ JwtAuthGuard }} from './guard';\n@Module({{ controllers: [AController], {saglayici} }})\nexport class AppModule {{}}\n"
            );
            let main = "import { NestFactory } from '@nestjs/core';\nimport { AppModule } from './app.module';\nNestFactory.create(AppModule);\n".to_string();
            vec![
                ("girdi.dto.ts".into(), dto),
                ("dogrulama.ts".into(), dogrulama.into()),
                ("guard.ts".into(), guard),
                ("a.controller.ts".into(), kontrolcu),
                ("app.module.ts".into(), modul),
                ("main.ts".into(), main),
            ]
        })
}

// ---- Rastgele kapı durumları (kart üretimini doğrudan sınar) ----

fn rastgele_kapi() -> impl Strategy<Value = Kapi> {
    let alan =
        (any::<bool>(), any::<bool>(), prop::collection::vec(sec(&["IsString()", "Min(1)", "IsOptional()", "Bilinmeyen(x)"]), 0..3)).prop_map(|(zorunlu, belirsiz, kurallar)| {
            Alan {
                tip: Some("string".into()),
                zorunlu,
                belirsiz,
                kurallar: kurallar.into_iter().map(String::from).collect(),
                satir: 1,
                kalitimdan: None,
                degerler: vec![],
                ic: None,
            }
        });
    let girdi = (prop::option::of((any::<bool>(), prop::collection::btree_map("[a-z]{1,4}", alan, 0..4))), prop::option::of("[a-z]{1,4}")).prop_map(|(sema, ad)| Girdi {
        ad,
        tip: Some("Girdi".into()),
        sema: sema.map(|(bulunamadi, alanlar)| Sema {
            adaylar: vec![],
            dizi: false,
            dto: "Girdi".into(),
            dosya: Some("a.ts".into()),
            bulunamadi,
            kalitim: None,
            alanlar: alanlar.into_iter().collect::<BTreeMap<_, _>>(),
        }),
    });
    (
        sec(&["GET", "POST", "PATCH", "DELETE"]),
        sec(&["gerekli", "herkese_acik", "istege_bagli"]),
        sec(&["kesin", "varsayim", "varsayim_bilinmeyen_guard", "varsayim_isaret"]),
        prop::option::of(Just("varsayim".to_string())),
        prop::collection::vec(girdi, 0..3),
        "[a-z]{1,6}",
    )
        .prop_map(|(yontem, giris, kanit, roller_kaniti, govde, ad)| Kapi {
            id: format!("{yontem} /{ad}"),
            yontem: yontem.into(),
            yol: format!("/{ad}"),
            kontrolcu: "AController".into(),
            fonksiyon: ad,
            dosya: "a.ts".into(),
            satir: 1,
            giris: giris.into(),
            guardlar: vec![],
            roller: if roller_kaniti.is_some() { vec!["a".into()] } else { vec![] },
            roller_kaniti,
            ozet: None,
            govde,
            sorgu: vec![],
            parametreler: vec![],
            kullanimdan_kalkmis: false,
            yerine: None,
            surum: None,
            uygulama: None,
            ana: true,
            giris_kaniti: kanit.into(),
            veri: None,
            yanit: None,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    /// 1 (kart düzeyi): rastgele kapı durumlarında belirsiz bir alt olgu işlemi asla "certain" yapmaz
    #[test]
    fn belirsiz_alt_olgu_ust_sonucu_kesin_yapmaz(kapilar in prop::collection::vec(rastgele_kapi(), 1..6)) {
        let kart = crate::benlik::kart("x", "1", "unknown", "test", &kapilar, "2026-01-01T00:00:00Z");
        prop_assert_eq!(kart_degismezi(&kapilar, &kart), Ok(()));
    }

    /// 1 + 2 + 3 (uçtan uca): rastgele NestJS kaynakları çökmeden, aynı sonuçla ve değişmezi bozmadan işlenir
    #[test]
    fn rastgele_uygulama_cokmez_belirleyici_ve_tutarli(dosyalar in uygulama()) {
        let (harita1, kart1, kapilar) = isle(&dosyalar);
        let (harita2, kart2, _) = isle(&dosyalar);
        prop_assert_eq!(&harita1, &harita2, "aynı girdi farklı harita");
        prop_assert_eq!(&kart1, &kart2, "aynı girdi farklı kart");
        prop_assert_eq!(kart_degismezi(&kapilar, &kart1), Ok(()));
    }

    /// 2: rastgele baytlar ve rastgele metin ayrıştırıcıyı ve adaptörü çökertmez
    #[test]
    fn rastgele_bayt_cokertmez(bayt in prop::collection::vec(any::<u8>(), 0..2048), metin in "\\PC{0,400}") {
        let _ = isle(&[("a.controller.ts".into(), String::from_utf8_lossy(&bayt).into_owned()), ("b.module.ts".into(), metin)]);
        let _ = crate::model::ayristir_metin(Path::new("/sanal/x.ts"), bayt);
    }
}

/// Proptest'in bulduğu durum (gerileme testi): aynı ad iki gövde şemasında, biri belirsiz biri kesin; kart zayıf damgayı korur
#[test]
fn ayni_adli_alan_zayif_damgayi_korur() {
    let alan = |belirsiz| Alan { tip: Some("string".into()), zorunlu: false, belirsiz, kurallar: vec![], satir: 1, kalitimdan: None, degerler: vec![], ic: None };
    let girdi = |belirsiz| Girdi {
        ad: None,
        tip: Some("Girdi".into()),
        sema: Some(Sema {
            adaylar: vec![],
            dizi: false,
            dto: "Girdi".into(),
            dosya: Some("a.ts".into()),
            bulunamadi: false,
            kalitim: None,
            alanlar: BTreeMap::from([("j".to_string(), alan(belirsiz))]),
        }),
    };
    for sira in [[true, false], [false, true]] {
        let kapi = Kapi {
            id: "GET /a".into(),
            yontem: "GET".into(),
            yol: "/a".into(),
            kontrolcu: "AController".into(),
            fonksiyon: "a".into(),
            dosya: "a.ts".into(),
            satir: 1,
            giris: "gerekli".into(),
            guardlar: vec![],
            roller: vec![],
            roller_kaniti: None,
            ozet: None,
            govde: vec![girdi(sira[0]), girdi(sira[1])],
            sorgu: vec![],
            parametreler: vec![],
            kullanimdan_kalkmis: false,
            yerine: None,
            surum: None,
            uygulama: None,
            ana: true,
            giris_kaniti: "kesin".into(),
            veri: None,
            yanit: None,
        };
        let kart = crate::benlik::kart("x", "1", "unknown", "test", std::slice::from_ref(&kapi), "2026-01-01T00:00:00Z");
        assert_eq!(kart["operations"][0]["inputs"]["j"]["stamp"], "inferred", "{sira:?}: {}", kart["operations"][0]["inputs"]);
        assert_eq!(kart_degismezi(&[kapi], &kart), Ok(()));
    }
}
