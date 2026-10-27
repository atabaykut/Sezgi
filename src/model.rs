//! TypeScript kaynak kodundan çatıdan bağımsız genel model çıkarır:
//! içe aktarmalar, sınıflar, dekoratörler, alanlar, metotlar ve parametreler.
//! Çatıya özgü anlam (rota, yetki, girdi şeması) bu modelin üstünde adaptörle verilir.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Serialize)]
pub struct Dekorator {
    pub ad: String,
    pub argumanlar: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ozellik {
    pub ad: String,
    pub tip: Option<String>,
    pub opsiyonel_isaret: bool,
    pub dekoratorler: Vec<Dekorator>,
    pub satir: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Parametre {
    pub ad: String,
    pub tip: Option<String>,
    pub dekoratorler: Vec<Dekorator>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metot {
    pub ad: String,
    pub dekoratorler: Vec<Dekorator>,
    pub parametreler: Vec<Parametre>,
    pub satir: usize,
    /// Gövdedeki üye çağrıları, noktalı yol olarak: "this.prisma.user.findMany", "tx.post.create"
    pub cagrilar: Vec<String>,
    /// Üstveri okumaları: `this.reflector.getAllAndOverride(ANAHTAR, ...)` -> ("this.reflector.getAllAndOverride", "ANAHTAR").
    /// Guard'ın hangi işareti (ör. herkese açık) okuduğu buradan çıkarılır.
    pub meta_okumalari: Vec<(String, String)>,
    /// Dönüş tipi bildirimi (`Promise<AlbumResponseDto>`), iki nokta olmadan
    pub donus_tipi: Option<String>,
    /// Davranış özeti öğeleri: fırlatmalar ve koşulları, iki yollu ifadeler, yumuşak silme işareti
    #[serde(skip)]
    pub dna: crate::dna::GovdeDna,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sinif {
    pub ad: String,
    pub dekoratorler: Vec<Dekorator>,
    pub kalitim: Option<String>,
    pub ozellikler: Vec<Ozellik>,
    pub metotlar: Vec<Metot>,
    pub satir: usize,
}

/// Zod şeması alanı: `z.string().min(1).optional()` gibi bir zincirin özeti
#[derive(Debug, Clone, Serialize)]
pub struct ZodAlan {
    pub tip: String,
    pub istege_bagli: bool,
    pub kurallar: Vec<String>,
    pub satir: usize,
    /// `z.enum([...])` / `z.nativeEnum(X)` / `z.enum(X)`: değer listesinin ham metni ya da enum adı (adaptör çözer)
    pub degerler_ham: Option<String>,
}

/// Zod şema ifadesi: nesne, başka şemaya atıf ya da türetme
#[derive(Debug, Clone, Serialize)]
pub enum ZodIfade {
    Nesne(Vec<(String, ZodAlan)>),
    Atif(String),
    Genislet(Box<ZodIfade>, Vec<(String, ZodAlan)>),
    Birlestir(Box<ZodIfade>, String),
    Kismi(Box<ZodIfade>),
    Sec(Box<ZodIfade>, Vec<String>),
    Haric(Box<ZodIfade>, Vec<String>),
    /// z.array(Şema): öğesi bir şema olan dizi (gövdenin kendisi dizi olabilir)
    Dizi(Box<ZodIfade>),
    Bilinmiyor,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZodSema {
    pub ad: String,
    pub ifade: ZodIfade,
    /// Şemanın kendisi `.optional()` gibi bir halkayla bitiyorsa, ona atıf yapan alan da isteğe bağlıdır
    pub istege_bagli: bool,
    pub satir: usize,
    /// Nesne olmayan şema (`const tarih = z.string().datetime()` ya da başka bir şemaya atıf): alanın tipi bu
    /// sabitin adıysa tip ve kurallar buradan, yani tanımdan okunur
    pub skaler: Option<ZodAlan>,
    /// Şemanın kendisine zincirlenmiş nesne düzeyi kurallar (`.superRefine(...)`, `.refine(...)`, `.check(...)`,
    /// `.transform(...)`): alanlar arası koşullar ya da dönüşümler. Alan listesini değiştirmezler ama önizlemede
    /// çalıştırılamazlar; adları burada tutulur ki "bilinen bütün kurallar geçer" denmesin
    pub nesne_kurallari: Vec<String>,
}

/// Bir fonksiyon gövdesindeki çağrı: `SetMetadata(KEY, true)`, `IsOptional()` gibi
#[derive(Debug, Clone, Serialize)]
pub struct Cagri {
    pub ad: String,
    pub argumanlar: Vec<String>,
    /// Çağrı her çalışmada gerçekleşmeyebilir: if, üçlü ifade, döngü, &&/||, iç fonksiyon içinde
    pub kosullu: bool,
}

/// Bir tanımın gövdesinde izlenecek en fazla çağrı (dev fonksiyon belleği doldurmasın)
const EN_FAZLA_CAGRI: usize = 256;

#[derive(Debug, Clone, Serialize)]
pub struct Dosya {
    pub yol: PathBuf,
    /// İçe aktarılan ad -> modül belirteci (ör. "./dto/kitap.dto")
    pub iceaktarmalar: HashMap<String, String>,
    /// İçe aktarma takma adı -> asıl ad: import { ApiProperty as DocsProperty } -> (DocsProperty, ApiProperty)
    pub iceaktarma_asil_adlari: HashMap<String, String>,
    pub siniflar: Vec<Sinif>,
    /// enum Ad { Uye = 'deger' } -> ("Ad.Uye", "deger")
    pub sabitler: HashMap<String, String>,
    pub zod_semalar: Vec<ZodSema>,
    /// const X = [A, B, ...Y] gibi ad dizileri (modül ve controller listeleri için)
    pub diziler: HashMap<String, Vec<String>>,
    /// export { Asil as TakmaAd } -> (TakmaAd, Asil)
    pub takma_adlar: HashMap<String, String>,
    /// Monorepo'da uygulamanın bağımlı olduğu kardeş paketten mi geldi (kapılar bu dosyalardan alınmaz)
    pub ek_paket: bool,
    /// const X = Y (ya da Y as T): sabitten sabite atıf -> (X, Y)
    pub sabit_atiflari: HashMap<String, String>,
    /// enum Ad { A = 'a', B = 'b' } -> (Ad, [a, b]); yalnızca bütün üyeler metin değerliyse (eksik liste verilmez)
    pub enumlar: HashMap<String, Vec<String>>,
    /// `type Girdi = A | B | C`: üyeleri yalnızca tip adı olan birleşimler (gövde birkaç biçimden biri olabilir)
    pub tip_birlesimleri: HashMap<String, Vec<String>>,
    /// Üst düzey fonksiyon ve sabit tanımlarının gövdesindeki çağrılar: özel dekoratörlerin anlamı
    /// adından değil tanımından çıkarılır (ör. `export const Public = () => SetMetadata(IS_PUBLIC, true)`)
    pub tanimlar: HashMap<String, Vec<Cagri>>,
    /// Üst düzey fonksiyonların davranış özeti öğeleri (servisin çağırdığı yardımcı: `requireAccess(...)`)
    #[serde(skip)]
    pub tanim_dna: HashMap<String, crate::dna::GovdeDna>,
    pub satir_sayisi: usize,
    /// Ayrıştırıcının hata bulduğu satırlar (kısmen okunan dosyalar için)
    pub hatali_satirlar: Vec<usize>,
    /// Kaynak metin: adaptör genel önek, kök modül ve genel guard aramak için dosyayı yeniden okumaz
    #[serde(skip)]
    pub metin: String,
}

fn metin(n: Node, kaynak: &[u8]) -> String {
    n.utf8_text(kaynak).unwrap_or("").to_string()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn dekorator(n: Node, k: &[u8]) -> Option<Dekorator> {
    let ic = n.named_child(0)?;
    match ic.kind() {
        "call_expression" => {
            let ad = ic.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
            let mut argumanlar = Vec::new();
            if let Some(args) = ic.child_by_field_name("arguments") {
                let mut c = args.walk();
                for a in args.named_children(&mut c) {
                    if a.kind() != "comment" {
                        argumanlar.push(metin(a, k));
                    }
                }
            }
            Some(Dekorator { ad, argumanlar })
        }
        _ => Some(Dekorator { ad: metin(ic, k), argumanlar: vec![] }),
    }
}

/// Bir düğümün içindeki "decorator" alanlarını toplar.
fn ic_dekoratorler(n: Node, k: &[u8]) -> Vec<Dekorator> {
    let mut c = n.walk();
    n.children_by_field_name("decorator", &mut c).filter_map(|d| dekorator(d, k)).collect()
}

fn tip_metni(n: Node, k: &[u8]) -> Option<String> {
    n.child_by_field_name("type").map(|t| metin(t, k).trim_start_matches(':').trim().to_string())
}

fn parametreler(m: Node, k: &[u8]) -> Vec<Parametre> {
    let mut sonuc = Vec::new();
    if let Some(ps) = m.child_by_field_name("parameters") {
        let mut c = ps.walk();
        for p in ps.named_children(&mut c) {
            if p.kind() != "required_parameter" && p.kind() != "optional_parameter" {
                continue;
            }
            let ad = p.child_by_field_name("pattern").map(|x| metin(x, k)).unwrap_or_default();
            sonuc.push(Parametre { ad, tip: tip_metni(p, k), dekoratorler: ic_dekoratorler(p, k) });
        }
    }
    sonuc
}

/// Reflector'ın üstveri okuyan yöntemleri
const META_OKUMA: [&str; 3] = ["get", "getAllAndOverride", "getAllAndMerge"];

/// Metot gövdesi tek geçişte okunur: davranış özeti öğeleri (`dna::ts_govde_ile`) ve aynı geçişte `a.b.c(...)` biçimli
/// çağrılar (iç içe olanlar dahil) noktalı yol olarak. Ara halkada çağrı olan zincirler (`a.b().c()`) yalnızca kendi
/// sade parçalarıyla kaydedilir. Ağaç yığınla gezilir (özyineleme yok): çok derin iç içe ifade yığını taşırmasın.
fn govde_oku(govde: Node, k: &[u8], out: &mut Vec<String>, meta: &mut Vec<(String, String)>) -> crate::dna::GovdeDna {
    crate::dna::ts_govde_ile(govde, k, &mut |n| cagri_topla(n, k, out, meta))
}

/// Tek düğüm için çağrı kaydı (bkz. `govde_oku`)
fn cagri_topla(n: Node, k: &[u8], out: &mut Vec<String>, meta: &mut Vec<(String, String)>) {
    if n.kind() != "call_expression" {
        return;
    }
    if let Some(f) = n.child_by_field_name("function").filter(|f| f.kind() == "member_expression") {
        let yol: String = metin(f, k).chars().filter(|c| !c.is_whitespace() && *c != '?' && *c != '!').collect();
        if yol.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')) {
            if META_OKUMA.iter().any(|m| yol.ends_with(&format!(".{m}"))) {
                if let Some(ilk) = n.child_by_field_name("arguments").and_then(|a| a.named_child(0)) {
                    meta.push((yol.clone(), metin(ilk, k)));
                }
            }
            if !out.contains(&yol) {
                out.push(yol);
            }
        }
    }
}

fn sinif(n: Node, k: &[u8], dis_dekoratorler: Vec<Dekorator>) -> Option<Sinif> {
    let ad = metin(n.child_by_field_name("name")?, k);
    let mut dekoratorler = dis_dekoratorler;
    dekoratorler.extend(ic_dekoratorler(n, k));
    let mut kalitim = None;
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        if ch.kind() == "class_heritage" {
            let mut c2 = ch.walk();
            for h in ch.named_children(&mut c2) {
                if h.kind() == "extends_clause" {
                    let t = metin(h, k);
                    kalitim = Some(t.trim_start_matches("extends").trim().to_string());
                }
            }
        }
    }
    let mut ozellikler = Vec::new();
    let mut metotlar = Vec::new();
    if let Some(govde) = n.child_by_field_name("body") {
        // Metot dekoratörleri class_body içinde metottan önce kardeş düğüm olarak gelir.
        let mut bekleyen: Vec<Dekorator> = Vec::new();
        let mut c3 = govde.walk();
        for uye in govde.named_children(&mut c3) {
            match uye.kind() {
                "decorator" => {
                    if let Some(d) = dekorator(uye, k) {
                        bekleyen.push(d);
                    }
                }
                "method_definition" => {
                    let mut ds = std::mem::take(&mut bekleyen);
                    ds.extend(ic_dekoratorler(uye, k));
                    let ad = uye.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
                    let (mut cagri, mut meta) = (Vec::new(), Vec::new());
                    let mut dna = crate::dna::GovdeDna::default();
                    if let Some(g) = uye.child_by_field_name("body") {
                        dna = govde_oku(g, k, &mut cagri, &mut meta);
                    }
                    let donus_tipi = uye.child_by_field_name("return_type").map(|t| metin(t, k).trim_start_matches(':').trim().to_string());
                    metotlar.push(Metot { ad, dekoratorler: ds, parametreler: parametreler(uye, k), satir: satir(uye), cagrilar: cagri, meta_okumalari: meta, donus_tipi, dna });
                }
                "public_field_definition" => {
                    let mut ds = std::mem::take(&mut bekleyen);
                    ds.extend(ic_dekoratorler(uye, k));
                    let ad = uye.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
                    let mut opsiyonel_isaret = false;
                    let mut c4 = uye.walk();
                    for t in uye.children(&mut c4) {
                        if t.kind() == "?" {
                            opsiyonel_isaret = true;
                        }
                    }
                    ozellikler.push(Ozellik { ad, tip: tip_metni(uye, k), opsiyonel_isaret, dekoratorler: ds, satir: satir(uye) });
                }
                "comment" => {}
                _ => bekleyen.clear(),
            }
        }
    }
    Some(Sinif { ad, dekoratorler, kalitim, ozellikler, metotlar, satir: satir(n) })
}

/// Zincirli çağrıyı çözer: a.b(x).c(y) -> (taban düğüm, [(yöntem, argüman düğümü)] içten dışa)
fn zincir<'t>(mut n: Node<'t>, k: &[u8]) -> (Node<'t>, Vec<(String, Option<Node<'t>>)>) {
    let mut halkalar = Vec::new();
    loop {
        if n.kind() == "call_expression" {
            if let Some(f) = n.child_by_field_name("function") {
                if f.kind() == "member_expression" {
                    let ad = f.child_by_field_name("property").map(|p| metin(p, k)).unwrap_or_default();
                    let arg = n.child_by_field_name("arguments").and_then(|a| a.named_child(0));
                    halkalar.push((ad, arg));
                    n = f.child_by_field_name("object").unwrap();
                    continue;
                }
            }
        }
        break;
    }
    halkalar.reverse();
    (n, halkalar)
}

const ZOD_ACIKLAMA: [&str; 6] = ["describe", "meta", "openapi", "brand", "readonly", "transform"];
/// Şekli değiştirmeyen ama değeri reddedebilen ya da dönüştüren nesne düzeyi halkalar (önizlemede çalıştırılmaz)
const ZOD_NESNE_KURALI: [&str; 5] = ["refine", "superRefine", "check", "transform", "pipe"];
const ZOD_ISTEGE_BAGLI: [&str; 4] = ["optional", "nullish", "default", "catch"];

pub(crate) fn zod_alan(n: Node, k: &[u8]) -> ZodAlan {
    let (taban, halkalar) = zincir(n, k);
    let mut tip = metin(taban, k);
    let mut kurallar = Vec::new();
    let mut istege_bagli = false;
    let mut degerler_ham = None;
    for (i, (ad, arg)) in halkalar.iter().enumerate() {
        // z.iso.datetime() / z.iso.date(): ISO biçimli metin; biçim kural olarak okunur (zod 4)
        if i == 0 && tip == "z.iso" {
            tip = "string".into();
            kurallar.push(format!("{}({})", ad, arg.map(|a| metin(a, k)).unwrap_or_default()));
            continue;
        }
        // z.codec(giriş, çıkış, ...): istekte gelen değer giriş şemasıdır
        if i == 0 && tip == "z" && ad == "codec" {
            if let Some(a) = arg {
                let ic = zod_alan(*a, k);
                tip = ic.tip;
                kurallar = ic.kurallar;
                degerler_ham = ic.degerler_ham;
                istege_bagli |= ic.istege_bagli;
            }
            continue;
        }
        // z.string() gibi ilk halka tipin kendisidir
        if i == 0 && tip == "z" {
            tip = ad.clone();
            if matches!(ad.as_str(), "enum" | "nativeEnum") {
                degerler_ham = arg.map(|a| metin(a, k));
            }
            // z.array(X): öğe tipi önizlemede kullanılır
            if ad == "array" {
                if let Some(a) = arg {
                    kurallar.push(format!("of({})", metin(*a, k)));
                }
            }
            continue;
        }
        if ad == "nullable" {
            kurallar.push("nullable()".into());
            continue;
        }
        if ZOD_ISTEGE_BAGLI.contains(&ad.as_str()) {
            istege_bagli = true;
            continue;
        }
        if ZOD_ACIKLAMA.contains(&ad.as_str()) {
            continue;
        }
        // X.array(): şimdiye kadarki tip ve kurallar öğeye aittir; alan dizidir ve sonraki halkalar diziye uygulanır
        if ad == "array" && tip != "array" {
            let oge = std::iter::once(format!("z.{tip}()")).chain(kurallar.iter().map(|r| format!(".{r}"))).collect::<String>();
            kurallar = vec![format!("of({oge})")];
            tip = "array".into();
            continue;
        }
        // X.or(Y) / X.and(Y): birleşim tipi koddan tek bir tipe indirgenemez; kurallar kontrol edilemez olarak kalır
        if matches!(ad.as_str(), "or" | "and") {
            tip = "union".into();
        }
        kurallar.push(format!("{}({})", ad, arg.map(|a| metin(a, k)).unwrap_or_default()));
    }
    ZodAlan { tip, istege_bagli, kurallar, satir: satir(n), degerler_ham }
}

fn zod_nesne(n: Node, k: &[u8]) -> Vec<(String, ZodAlan)> {
    let mut alanlar = Vec::new();
    if n.kind() != "object" {
        return alanlar;
    }
    let mut c = n.walk();
    for p in n.named_children(&mut c) {
        // ...BaseSchema.shape ya da ...BaseSchema: başka şemanın alanlarını içeri yayar
        if p.kind() == "spread_element" {
            if let Some(ic) = p.named_child(0) {
                let t = metin(ic, k);
                let ad = t.trim_end_matches(".shape").to_string();
                alanlar.push((format!("...{ad}"), ZodAlan { tip: ad, istege_bagli: false, kurallar: vec![], satir: satir(p), degerler_ham: None }));
            }
            continue;
        }
        if p.kind() == "pair" {
            let anahtar = p.child_by_field_name("key").map(|x| metin(x, k).trim_matches(|ch| ch == '\'' || ch == '"').to_string());
            if let (Some(a), Some(v)) = (anahtar, p.child_by_field_name("value")) {
                alanlar.push((a, zod_alan(v, k)));
            }
        }
    }
    alanlar
}

fn anahtar_listesi(n: Option<Node>, k: &[u8]) -> Vec<String> {
    let Some(n) = n else { return vec![] };
    let mut c = n.walk();
    n.named_children(&mut c).filter_map(|p| p.child_by_field_name("key").or(Some(p)).map(|x| metin(x, k).trim_matches(|ch| ch == '\'' || ch == '"').to_string())).collect()
}

/// Sarmalayıcı fonksiyonların (sarmala(sarmala(...))) izlenecek en fazla derinliği
const EN_DERIN_SARMAL: usize = 16;

/// `nesne`: şemanın kendisine zincirlenmiş nesne düzeyi kural adları (ör. `superRefine`) buraya eklenir
fn zod_ifade(n: Node, k: &[u8], derinlik: usize, nesne: &mut Vec<String>) -> ZodIfade {
    let (taban, halkalar) = zincir(n, k);
    let mut ifade = match taban.kind() {
        "identifier" if metin(taban, k) != "z" => ZodIfade::Atif(metin(taban, k)),
        "identifier" => ZodIfade::Bilinmiyor,
        // Sarmalayıcı fonksiyon: sarmala(Şema) -> şeklin Şema'dan geldiği varsayılır
        "call_expression" => {
            let fonk = taban.child_by_field_name("function").filter(|f| f.kind() == "identifier").map(|f| metin(f, k));
            match (fonk, taban.child_by_field_name("arguments").and_then(|a| a.named_child(0))) {
                // sarmala({ alanlar }) -> nesne; adında "partial" geçen sarmalayıcı alanları isteğe bağlı yapar
                (Some(f), Some(ilk)) if ilk.kind() == "object" => {
                    let n = ZodIfade::Nesne(zod_nesne(ilk, k));
                    if f.to_lowercase().contains("partial") {
                        ZodIfade::Kismi(Box::new(n))
                    } else {
                        n
                    }
                }
                (Some(_), Some(ilk)) if derinlik < EN_DERIN_SARMAL => zod_ifade(ilk, k, derinlik + 1, nesne),
                _ => return ZodIfade::Bilinmiyor,
            }
        }
        _ => return ZodIfade::Bilinmiyor,
    };
    for (ad, arg) in halkalar {
        ifade = match (ad.as_str(), ifade) {
            ("object" | "strictObject" | "looseObject", ZodIfade::Bilinmiyor) => ZodIfade::Nesne(arg.map(|a| zod_nesne(a, k)).unwrap_or_default()),
            ("array", ZodIfade::Bilinmiyor) if derinlik < EN_DERIN_SARMAL => match arg.map(|a| zod_ifade(a, k, derinlik + 1, &mut Vec::new())) {
                Some(ZodIfade::Bilinmiyor) | None => ZodIfade::Bilinmiyor,
                Some(oge) => ZodIfade::Dizi(Box::new(oge)),
            },
            ("extend", onceki) => ZodIfade::Genislet(Box::new(onceki), arg.map(|a| zod_nesne(a, k)).unwrap_or_default()),
            ("merge", onceki) => ZodIfade::Birlestir(Box::new(onceki), arg.map(|a| metin(a, k)).unwrap_or_default()),
            ("partial", onceki) => ZodIfade::Kismi(Box::new(onceki)),
            ("pick", onceki) => ZodIfade::Sec(Box::new(onceki), anahtar_listesi(arg, k)),
            ("omit", onceki) => ZodIfade::Haric(Box::new(onceki), anahtar_listesi(arg, k)),
            // refine, superRefine, check, transform: şekli değiştirmez ama önizlemede çalıştırılamaz; adı kaydedilir
            (ad, onceki) if ZOD_NESNE_KURALI.contains(&ad) => {
                if !nesne.iter().any(|x| x == ad) {
                    nesne.push(ad.to_string());
                }
                onceki
            }
            (_, onceki) => onceki, // meta, describe, strict vb. şekli değiştirmez
        };
    }
    ifade
}

fn sabitleri_topla(n: Node, k: &[u8], out: &mut HashMap<String, String>, enumlar: &mut HashMap<String, Vec<String>>) {
    // enum Ad { Uye = 'deger' }
    if n.kind() == "enum_declaration" {
        let ad = n.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
        if let Some(govde) = n.child_by_field_name("body") {
            let (mut degerler, mut tam) = (Vec::new(), true);
            let mut c = govde.walk();
            for u in govde.named_children(&mut c) {
                if u.kind() == "comment" {
                    continue;
                }
                // Düz metin: 'x' / "x" ya da içinde ${...} olmayan `x` (template_substitution yoksa değer sabittir)
                let duz_metin = |v: Node| {
                    v.kind() == "string"
                        || (v.kind() == "template_string" && {
                            let mut w = v.walk();
                            let sabit = v.named_children(&mut w).all(|c| c.kind() != "template_substitution");
                            sabit
                        })
                };
                let metinli = u.kind() == "enum_assignment" && u.child_by_field_name("value").map(duz_metin).unwrap_or(false);
                if !metinli {
                    tam = false;
                }
                if u.kind() == "enum_assignment" {
                    let uye = u.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
                    if let Some(v) = u.child_by_field_name("value") {
                        if duz_metin(v) {
                            let deger = metin(v, k).trim_matches(|ch| ch == '\'' || ch == '"' || ch == '`').to_string();
                            degerler.push(deger.clone());
                            out.insert(format!("{ad}.{uye}"), deger);
                        }
                    }
                }
            }
            if tam && !degerler.is_empty() {
                enumlar.insert(ad, degerler);
            }
        }
    }
}

/// `A | B | C` birleşiminin üyeleri; bir üye yalnızca tip adı değilse (ör. `string`, `{ ... }`, `X<T>`) None
fn birlesim_uyeleri(n: Node, k: &[u8]) -> Option<Vec<String>> {
    if n.kind() != "union_type" {
        return None;
    }
    let mut uyeler = vec![];
    let mut yigin = vec![n];
    while let Some(d) = yigin.pop() {
        match d.kind() {
            "union_type" => {
                let mut c = d.walk();
                let mut alt: Vec<Node> = d.named_children(&mut c).collect();
                alt.reverse();
                yigin.extend(alt);
            }
            "type_identifier" => uyeler.push(metin(d, k)),
            _ => return None,
        }
        if uyeler.len() > 16 {
            return None;
        }
    }
    (uyeler.len() >= 2).then_some(uyeler)
}

/// Bir tanımın gövdesindeki bütün çağrılar ve koşullu olup olmadıkları (yığınla gezilir)
fn tanim_cagrilari(govde: Node, k: &[u8]) -> Vec<Cagri> {
    let mut out = Vec::new();
    let mut yigin = vec![(govde, false)];
    while let Some((n, kosullu)) = yigin.pop() {
        if out.len() >= EN_FAZLA_CAGRI {
            break;
        }
        if n.kind() == "call_expression" {
            if let Some(f) = n.child_by_field_name("function") {
                let ad: String = metin(f, k).chars().filter(|c| !c.is_whitespace()).collect();
                let mut argumanlar = Vec::new();
                if let Some(a) = n.child_by_field_name("arguments") {
                    let mut c = a.walk();
                    argumanlar = a.named_children(&mut c).filter(|x| x.kind() != "comment").map(|x| metin(x, k)).collect();
                }
                out.push(Cagri { ad, argumanlar, kosullu });
            }
        }
        // Bu düğümün altındaki çağrılar her zaman çalışmayabilir
        let kosul = kosullu
            || (n.id() != govde.id()
                && (matches!(
                    n.kind(),
                    "if_statement"
                        | "ternary_expression"
                        | "switch_statement"
                        | "for_statement"
                        | "for_in_statement"
                        | "while_statement"
                        | "do_statement"
                        | "catch_clause"
                        | "arrow_function"
                        | "function_expression"
                        | "function_declaration"
                ) || (n.kind() == "binary_expression" && n.child_by_field_name("operator").map(|o| matches!(o.kind(), "&&" | "||" | "??")).unwrap_or(false))));
        let mut c = n.walk();
        let cocuklar: Vec<Node> = n.named_children(&mut c).collect();
        yigin.extend(cocuklar.into_iter().rev().map(|x| (x, kosul)));
    }
    out
}

/// Kaynağın, yorum düğümleri boşluğa çevrilmiş kopyası (satır sonları korunur); ağaç imleçle gezilir
fn yorumlari_sil(kaynak: &[u8], kok: Node) -> Vec<u8> {
    let mut out = kaynak.to_vec();
    let mut c = kok.walk();
    loop {
        let n = c.node();
        if n.kind() == "comment" {
            for b in &mut out[n.start_byte()..n.end_byte()] {
                if *b != b'\n' && *b != b'\r' {
                    *b = b' ';
                }
            }
        } else if c.goto_first_child() {
            continue;
        }
        while !c.goto_next_sibling() {
            if !c.goto_parent() {
                return out;
            }
        }
    }
}

/// Ayrıştırıcının hata bulduğu satırlar; ağaç yığınla gezilir (çok derin dosyada yığın taşmasın)
fn hatalari_topla(kok: Node, out: &mut Vec<usize>) {
    let mut yigin = vec![kok];
    while let Some(n) = yigin.pop() {
        if n.is_error() || n.is_missing() {
            out.push(satir(n));
            continue;
        }
        if !n.has_error() {
            continue;
        }
        let mut c = n.walk();
        yigin.extend(n.children(&mut c));
    }
}

/// Bellekteki kaynağı ayrıştırır (`yol` yalnızca dosya adı ve dil seçimi için); fuzz ve özellik testleri de kullanır
pub fn ayristir_metin(yol: &Path, kaynak: Vec<u8>) -> Option<Dosya> {
    let mut p = Parser::new();
    let dil = if yol.extension().map(|e| e == "tsx").unwrap_or(false) { tree_sitter_typescript::LANGUAGE_TSX } else { tree_sitter_typescript::LANGUAGE_TYPESCRIPT };
    p.set_language(&dil.into()).ok()?;
    let agac = p.parse(&kaynak, None)?;
    let kok = agac.root_node();
    // Yorumlar boşlukla silinir (bayt konumları ve satırlar korunur): yorumdaki kod (ör. `// providers: [APP_GUARD ...]`)
    // hiçbir metin aramasında ya da dekoratör argümanında görünmez
    let yorumsuz = yorumlari_sil(&kaynak, kok);
    let k = yorumsuz.as_slice();

    let mut iceaktarmalar = HashMap::new();
    let mut iceaktarma_asil_adlari = HashMap::new();
    let mut siniflar = Vec::new();
    let mut sabitler = HashMap::new();
    let mut enumlar = HashMap::new();
    let mut zod_semalar = Vec::new();
    let mut diziler = HashMap::new();
    let mut takma_adlar = HashMap::new();
    let mut sabit_atiflari = HashMap::new();
    let mut tanimlar = HashMap::new();
    let mut tanim_dna = HashMap::new();
    let mut tip_birlesimleri = HashMap::new();
    let mut c = kok.walk();
    for st0 in kok.named_children(&mut c) {
        // export { Asil as TakmaAd }
        if st0.kind() == "export_statement" && st0.child_by_field_name("declaration").is_none() {
            let mut c2 = st0.walk();
            for ch in st0.named_children(&mut c2) {
                if ch.kind() == "export_clause" {
                    let mut c3 = ch.walk();
                    for sp in ch.named_children(&mut c3) {
                        if let (Some(ad), Some(takma)) = (sp.child_by_field_name("name"), sp.child_by_field_name("alias")) {
                            takma_adlar.insert(metin(takma, k), metin(ad, k));
                        }
                    }
                }
            }
        }
        // export const / export enum: dışa aktarılan bildirimi aç
        let st = if st0.kind() == "export_statement"
            && st0.child_by_field_name("declaration").map(|d| d.kind() != "class_declaration" && d.kind() != "abstract_class_declaration").unwrap_or(false)
        {
            st0.child_by_field_name("declaration").unwrap()
        } else {
            st0
        };
        sabitleri_topla(st, k, &mut sabitler, &mut enumlar);
        if st.kind() == "type_alias_declaration" {
            if let (Some(ad), Some(deger)) = (st.child_by_field_name("name"), st.child_by_field_name("value")) {
                if let Some(uyeler) = birlesim_uyeleri(deger, k) {
                    tip_birlesimleri.insert(metin(ad, k), uyeler);
                }
            }
        }
        if matches!(st.kind(), "function_declaration" | "generator_function_declaration") {
            if let (Some(ad), Some(g)) = (st.child_by_field_name("name"), st.child_by_field_name("body")) {
                tanimlar.insert(metin(ad, k), tanim_cagrilari(g, k));
                tanim_dna.insert(metin(ad, k), crate::dna::ts_govde(g, k));
            }
        }
        if st.kind() == "lexical_declaration" {
            let mut c2 = st.walk();
            for d in st.named_children(&mut c2) {
                if d.kind() != "variable_declarator" {
                    continue;
                }
                let (Some(ad), Some(deger)) = (d.child_by_field_name("name"), d.child_by_field_name("value")) else { continue };
                let ad = metin(ad, k);
                // X as T, X satisfies T, X! -> içteki ifade
                let mut deger = deger;
                while matches!(deger.kind(), "as_expression" | "satisfies_expression" | "non_null_expression" | "parenthesized_expression") {
                    match deger.named_child(0) {
                        Some(ic) => deger = ic,
                        None => break,
                    }
                }
                // const Ad = () => ... / function () {...} / Cagri(...): özel dekoratör olabilir, gövdesindeki çağrılar saklanır
                match deger.kind() {
                    "arrow_function" | "function_expression" | "function" => {
                        if let Some(g) = deger.child_by_field_name("body") {
                            tanimlar.insert(ad.clone(), tanim_cagrilari(g, k));
                            tanim_dna.insert(ad.clone(), crate::dna::ts_govde(g, k));
                        }
                    }
                    "call_expression" => {
                        tanimlar.insert(ad.clone(), tanim_cagrilari(deger, k));
                    }
                    _ => {}
                }
                match deger.kind() {
                    "identifier" | "member_expression" => {
                        // const A = B: B bir zod şemasıysa A da odur (yalnızca zod yollarından bakılır)
                        if deger.kind() == "identifier" {
                            zod_semalar.push(ZodSema {
                                ad: ad.clone(),
                                ifade: ZodIfade::Atif(metin(deger, k)),
                                istege_bagli: false,
                                satir: satir(d),
                                skaler: Some(zod_alan(deger, k)),
                                nesne_kurallari: vec![],
                            });
                        }
                        sabit_atiflari.insert(ad, metin(deger, k));
                    }
                    "string" => {
                        sabitler.insert(ad, metin(deger, k).trim_matches(|ch| ch == '\'' || ch == '"' || ch == '`').to_string());
                    }
                    "call_expression" => {
                        let mut nesne_kurallari = Vec::new();
                        let ifade = zod_ifade(deger, k, 0, &mut nesne_kurallari);
                        let (taban, halkalar) = zincir(deger, k);
                        let zod_zinciri = metin(taban, k) == "z" && !halkalar.is_empty();
                        if zod_zinciri || !matches!(ifade, ZodIfade::Bilinmiyor) {
                            let istege_bagli = halkalar.iter().any(|(h, _)| ZOD_ISTEGE_BAGLI.contains(&h.as_str()));
                            let skaler = matches!(ifade, ZodIfade::Bilinmiyor | ZodIfade::Atif(_) | ZodIfade::Dizi(_)).then(|| zod_alan(deger, k));
                            zod_semalar.push(ZodSema { ad, ifade, istege_bagli, satir: satir(d), skaler, nesne_kurallari });
                        }
                    }
                    "array" => {
                        let mut c3 = deger.walk();
                        let ogeler: Vec<String> = deger
                            .named_children(&mut c3)
                            .map(|o| if o.kind() == "spread_element" { format!("...{}", o.named_child(0).map(|x| metin(x, k)).unwrap_or_default()) } else { metin(o, k) })
                            .collect();
                        diziler.insert(ad, ogeler);
                    }
                    _ => {}
                }
            }
        }
        match st.kind() {
            "import_statement" => {
                let kaynak_modul = st.child_by_field_name("source").map(|s| metin(s, k).trim_matches(|ch| ch == '\'' || ch == '"' || ch == '`').to_string());
                if let Some(modul) = kaynak_modul {
                    // import { A, B as C } from '...'
                    let mut stack = vec![st];
                    while let Some(n) = stack.pop() {
                        if n.kind() == "import_specifier" {
                            let ad = n.child_by_field_name("alias").or_else(|| n.child_by_field_name("name"));
                            if let Some(ad) = ad {
                                iceaktarmalar.insert(metin(ad, k), modul.clone());
                            }
                            // import { IsOptional as Secimli }: takma adın asıl adı (dekoratör anlamı asıl addan çıkar)
                            if let (Some(takma), Some(asil)) = (n.child_by_field_name("alias"), n.child_by_field_name("name")) {
                                iceaktarma_asil_adlari.insert(metin(takma, k), metin(asil, k));
                            }
                        } else {
                            let mut c2 = n.walk();
                            stack.extend(n.named_children(&mut c2));
                        }
                    }
                }
            }
            "export_statement" => {
                let dis = ic_dekoratorler(st, k);
                if let Some(d) = st.child_by_field_name("declaration") {
                    if d.kind() == "class_declaration" || d.kind() == "abstract_class_declaration" {
                        if let Some(s) = sinif(d, k, dis) {
                            siniflar.push(s);
                        }
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(s) = sinif(st, k, vec![]) {
                    siniflar.push(s);
                }
            }
            _ => {}
        }
    }
    let mut hatali_satirlar = Vec::new();
    hatalari_topla(kok, &mut hatali_satirlar);
    hatali_satirlar.sort_unstable();
    hatali_satirlar.dedup();
    let satir_sayisi = kaynak.iter().filter(|b| **b == b'\n').count() + 1;
    Some(Dosya {
        yol: yol.to_path_buf(),
        iceaktarmalar,
        iceaktarma_asil_adlari,
        siniflar,
        sabitler,
        zod_semalar,
        diziler,
        takma_adlar,
        ek_paket: false,
        sabit_atiflari,
        enumlar,
        tip_birlesimleri,
        tanimlar,
        tanim_dna,
        satir_sayisi,
        hatali_satirlar,
        metin: String::from_utf8_lossy(&yorumsuz).into_owned(),
    })
}
