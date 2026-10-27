//! Statik davranış özeti (DNA): işlemin gövdesinden ve çağırdığı servis / bağımlılık fonksiyonlarından okunan
//! - hata senaryoları: koşulu ve HTTP koduyla fırlatılan istisnalar (`throw new ForbiddenException()`,
//!   `raise HTTPException(status_code=403)`, `res.status(404)`); koşul kaynak koddaki ifadenin kısaltılmış metnidir
//!   (istek değeri içermez, anlamlandırılmaz),
//! - nesne düzeyi sahiplik denetimi: "kaydın sahibi çağıran mı" karşılaştırması (`item.owner_id != current_user.id`),
//! - davranışı ikiye bölen koşullar: `force ? Deleted : Trashed` gibi iki yollu ifadeler,
//! - yumuşak silme işaretleri (`deletedAt`, `isTrashed`, `softDelete`),
//! - dolaylı veri etkisi: ORM şemasındaki kaskad silmeler (Prisma `onDelete: Cascade`, SQLModel / SQLAlchemy
//!   `ondelete="CASCADE"`, `cascade_delete=True`, `cascade="all, delete"`, TypeORM `onDelete: 'CASCADE'`).
//!
//! Güven: desen kaynakta görüldüğü için doğrudan işlem gövdesindeki fırlatma `certain`; servis zinciri ya da
//! bağımlılık üzerinden ulaşılan fırlatma `inferred` (çağrının o yoldan geçtiği çözümlemeye dayanır). Kaskad şemadan
//! okunur ama silmenin o ilişkiyi tetiklediği çıkarımdır: `inferred`. Bulunamayan öğe "yok" demek değildir: kapsam
//! her zaman `acik`.
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

/// Bir gövdede tutulacak en fazla öğe (dev fonksiyonlar belleği doldurmasın)
const EN_FAZLA: usize = 32;
/// Koşul metninin en fazla uzunluğu
const KOSUL_UZUNLUGU: usize = 120;

/// Fırlatılan hata: HTTP kodu (biliniyorsa), istisna adı, koşulu ve satırı
#[derive(Debug, Clone, Serialize)]
pub struct Firlatma {
    pub durum: Option<u16>,
    pub istisna: String,
    pub kosul: Option<String>,
    pub satir: usize,
    /// Koşul kaydın sahibini çağıranla karşılaştırıyor
    pub sahiplik: bool,
}

/// Davranışı ikiye bölen koşul: `kosul ? evet : hayir`; `baglam` atandığı alan ya da geçtiği çağrı
#[derive(Debug, Clone, Serialize)]
pub struct Dal {
    pub kosul: String,
    pub evet: String,
    pub hayir: String,
    pub baglam: String,
    pub satir: usize,
}

/// Bir fonksiyon gövdesinden okunan davranış öğeleri
#[derive(Debug, Clone, Default, Serialize)]
pub struct GovdeDna {
    pub firlatmalar: Vec<Firlatma>,
    pub dallar: Vec<Dal>,
    /// Yumuşak silme işaretinin ilk görüldüğü satır
    pub yumusak: Option<usize>,
    /// Gövdede adıyla çağrılan fonksiyonlar (`deleteArticle(...)`, `crud.create_item(...)`): zincir için
    pub cagrilar: Vec<String>,
    /// ORM ile silinen modeller (`prisma.makale.delete(...)` -> "makale"): kaskad için
    pub silinen: Vec<String>,
    /// Yayılan olaylar ve kuyruğa konan işler: (tür "event" | "job", ad, satır)
    pub tetikler: Vec<(String, String, usize)>,
}

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).collect()
}

/// Kod metnini tek satıra indirip kısaltır
pub(crate) fn kisa(s: &str) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Yalnızca bütün ifadeyi saran parantez atılır (TypeScript `if (...)` koşulu); iç parantezlere dokunulmaz
    let s = match s.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(ic)
            if ic
                .chars()
                .scan(0i32, |d, c| {
                    *d += match c {
                        '(' => 1,
                        ')' => -1,
                        _ => 0,
                    };
                    Some(*d)
                })
                .all(|d| d >= 0) =>
        {
            ic.to_string()
        }
        _ => s,
    };
    if s.chars().count() > KOSUL_UZUNLUGU {
        format!("{}…", s.chars().take(KOSUL_UZUNLUGU).collect::<String>())
    } else {
        s
    }
}

/// NestJS yerleşik istisnaları ve HttpStatus adları -> HTTP kodu
fn istisna_kodu(ad: &str) -> Option<u16> {
    let ad = ad.rsplit('.').next().unwrap_or(ad);
    Some(match ad {
        "BadRequestException" | "BAD_REQUEST" => 400,
        "UnauthorizedException" | "UNAUTHORIZED" => 401,
        "ForbiddenException" | "FORBIDDEN" => 403,
        "NotFoundException" | "NOT_FOUND" => 404,
        "MethodNotAllowedException" | "METHOD_NOT_ALLOWED" => 405,
        "NotAcceptableException" | "NOT_ACCEPTABLE" => 406,
        "RequestTimeoutException" | "REQUEST_TIMEOUT" => 408,
        "ConflictException" | "CONFLICT" => 409,
        "GoneException" | "GONE" => 410,
        "PreconditionFailedException" | "PRECONDITION_FAILED" => 412,
        "PayloadTooLargeException" | "PAYLOAD_TOO_LARGE" => 413,
        "UnsupportedMediaTypeException" | "UNSUPPORTED_MEDIA_TYPE" => 415,
        "ImATeapotException" | "I_AM_A_TEAPOT" => 418,
        "UnprocessableEntityException" | "UNPROCESSABLE_ENTITY" => 422,
        "TOO_MANY_REQUESTS" => 429,
        "InternalServerErrorException" | "INTERNAL_SERVER_ERROR" => 500,
        "NotImplementedException" | "NOT_IMPLEMENTED" => 501,
        "BadGatewayException" | "BAD_GATEWAY" => 502,
        "ServiceUnavailableException" | "SERVICE_UNAVAILABLE" => 503,
        "GatewayTimeoutException" | "GATEWAY_TIMEOUT" => 504,
        _ => return None,
    })
}

/// İfadeden HTTP kodu: `403`, `HttpStatus.FORBIDDEN`, `status.HTTP_403_FORBIDDEN`
pub(crate) fn kod_ifadesi(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u16>() {
        return (100..600).contains(&n).then_some(n);
    }
    let son = s.rsplit('.').next().unwrap_or(s);
    if let Some(r) = son.strip_prefix("HTTP_") {
        return r.split('_').next().and_then(|n| n.parse().ok());
    }
    istisna_kodu(son)
}

/// Sahiplik karşılaştırması: bir taraf kaydın sahip alanı, diğer taraf istekteki kimlik (kural `cagri` modülünde, çağrı
/// grafiğinin derin analiziyle ortak). Kayıt kimliğinin yol parametresiyle karşılaştırılması (409 benzersizlik) ya da
/// çağıranın kendisiyle karşılaştırılması ("kendi hesabını silemezsin") sahiplik sayılmaz
pub(crate) fn sahiplik_mi(kosul: &str, durum: Option<u16>) -> bool {
    crate::cagri::sahiplik_karsilastirmasi(kosul, durum)
}

/// Düğümün içinde bulunduğu koşul: en yakın `if` (else dalındaysa olumsuzu), `catch` / `except`
pub(crate) fn kosul_bul(mut n: Node, kok: Node, k: &[u8], python: bool) -> Option<String> {
    while let Some(ust) = n.parent() {
        if n.id() == kok.id() {
            break;
        }
        match ust.kind() {
            "if_statement" => {
                let kosul = ust.child_by_field_name("condition").map(|c| kisa(&metin(c, k)))?;
                let olumlu = ust.child_by_field_name("consequence").is_some_and(|c| c.id() == n.id());
                return Some(if olumlu || n.kind() == "condition" { kosul } else { format!("!({kosul})") });
            }
            "elif_clause" => return ust.child_by_field_name("condition").map(|c| kisa(&metin(c, k))),
            "catch_clause" => return Some("catch".into()),
            "except_clause" => return Some("except".into()),
            "else_clause" if python => {
                // if ... else: üstteki if'in olumsuzu
                let ifd = ust.parent()?;
                let kosul = ifd.child_by_field_name("condition").map(|c| kisa(&metin(c, k)))?;
                return Some(format!("not ({kosul})"));
            }
            _ => {}
        }
        n = ust;
    }
    None
}

/// Basit dal ifadesi: ad, üye erişimi, metin, sayı, sabit
fn basit_mi(n: Node) -> bool {
    matches!(
        n.kind(),
        "identifier" | "member_expression" | "attribute" | "string" | "number" | "integer" | "true" | "false" | "null" | "none" | "True" | "False" | "None" | "undefined"
    )
}

/// İki yollu ifadenin bağlamı: atandığı alan (`status:`), değişken ya da içinde geçtiği çağrı
fn dal_baglami(n: Node, k: &[u8]) -> String {
    let mut x = n;
    for _ in 0..4 {
        let Some(u) = x.parent() else { break };
        match u.kind() {
            "pair" | "keyword_argument" => return u.child_by_field_name("key").or_else(|| u.child_by_field_name("name")).map(|a| metin(a, k)).unwrap_or_default(),
            "variable_declarator" | "assignment" | "assignment_expression" => {
                return u.child_by_field_name("name").or_else(|| u.child_by_field_name("left")).map(|a| metin(a, k)).unwrap_or_default()
            }
            "call_expression" | "call" => return u.child_by_field_name("function").map(|f| kisa(&metin(f, k))).unwrap_or_default(),
            _ => x = u,
        }
    }
    String::new()
}

/// Yumuşak silme alanları: yalnızca yazıldıklarında (silinme zamanı ya da `true` atanınca) işaret sayılır; okumak
/// (yanıt eşleme, filtre) silme değildir
const YUMUSAK_ALAN: [&str; 8] = ["deletedAt", "deleted_at", "trashedAt", "trashed_at", "isTrashed", "is_trashed", "isDeleted", "is_deleted"];
/// Yumuşak silme çağrıları (TypeORM `softDelete` / `softRemove`, yaygın yardımcı adları)
const YUMUSAK_CAGRI: [&str; 4] = ["softDelete", "softRemove", "soft_delete", "softDeleteMany"];

/// `alan = değer` / `alan: değer` yumuşak silme yazması mı: alan adı listede, değer zaman ya da `true`
fn yumusak_yazma(alan: &str, deger: &str) -> bool {
    let alan = alan.rsplit('.').next().unwrap_or(alan).trim();
    let d = deger.trim();
    YUMUSAK_ALAN.contains(&alan) && (d.contains("Date") || d.contains("now") || d == "true" || d == "True" || d.contains("datetime"))
}

/// Düğümün adlı çocuklarını yığına kaynak sırasıyla alınacak biçimde ekler (ara liste ayırmadan)
fn yigina_ekle<'t>(yigin: &mut Vec<Node<'t>>, n: Node<'t>) {
    let bas = yigin.len();
    let mut w = n.walk();
    yigin.extend(n.named_children(&mut w));
    yigin[bas..].reverse();
}

/// TypeScript / JavaScript fonksiyon gövdesi (tree-sitter-typescript düğümü)
pub fn ts_govde(govde: Node, k: &[u8]) -> GovdeDna {
    ts_govde_ile(govde, k, &mut |_| {})
}

/// `ts_govde`, aynı geçişte her düğümü `ziyaret`e de verir: gövde bir kez dolaşılır (model çağrı listesini aynı
/// geçişte toplar)
pub fn ts_govde_ile(govde: Node, k: &[u8], ziyaret: &mut dyn FnMut(Node)) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![govde];
    while let Some(n) = yigin.pop() {
        ziyaret(n);
        match n.kind() {
            "throw_statement" if d.firlatmalar.len() < EN_FAZLA => {
                if let Some(yeni) = n.named_child(0).filter(|x| x.kind() == "new_expression") {
                    let ad = yeni.child_by_field_name("constructor").map(|c| metin(c, k)).unwrap_or_default();
                    let args: Vec<String> = yeni.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().map(|a| metin(a, k)).collect();
                    let durum = istisna_kodu(&ad).or_else(|| args.iter().take(2).find_map(|a| kod_ifadesi(a)));
                    if durum.is_some() || ad.ends_with("Exception") || ad.ends_with("Error") {
                        let kosul = kosul_bul(n, govde, k, false);
                        let sahiplik = kosul.as_deref().is_some_and(|c| sahiplik_mi(c, durum));
                        d.firlatmalar.push(Firlatma { durum, istisna: ad, kosul, satir: n.start_position().row + 1, sahiplik });
                    }
                }
            }
            "call_expression" => {
                let f = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                let args: Vec<Node> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                let son = f.rsplit('.').next().unwrap_or("");
                // res.status(404) / res.sendStatus(403) / createError(404)
                let kod = match son {
                    "status" | "sendStatus" if f.contains('.') => args.first().and_then(|a| kod_ifadesi(&metin(*a, k))),
                    "createError" | "createHttpError" | "httpError" => args.first().and_then(|a| kod_ifadesi(&metin(*a, k))),
                    _ => None,
                };
                if let Some(kod) = kod.filter(|c| *c >= 400) {
                    if d.firlatmalar.len() < EN_FAZLA {
                        let kosul = kosul_bul(n, govde, k, false);
                        let sahiplik = kosul.as_deref().is_some_and(|c| sahiplik_mi(c, Some(kod)));
                        d.firlatmalar.push(Firlatma {
                            durum: Some(kod),
                            istisna: f.chars().filter(|c| !c.is_whitespace()).collect(),
                            kosul,
                            satir: n.start_position().row + 1,
                            sahiplik,
                        });
                    }
                }
                let parcalar: Vec<&str> = f.split('.').map(str::trim).collect();
                if d.yumusak.is_none() && parcalar.last().is_some_and(|s| YUMUSAK_CAGRI.contains(s)) {
                    d.yumusak = Some(n.start_position().row + 1);
                }
                // Olay ve iş: `emit('X')` / `emit(kosul ? 'X' : 'Y')`, `queue({ name: JobName.X })`, `queueAll([...])`,
                // `xQueue.add('ad', ...)`
                if d.tetikler.len() < EN_FAZLA {
                    let mut ekle = |tur: &str, ad: String| {
                        if !ad.is_empty() && !d.tetikler.iter().any(|(t, a, _)| t == tur && *a == ad) {
                            d.tetikler.push((tur.to_string(), ad, n.start_position().row + 1));
                        }
                    };
                    match son {
                        "emit" | "emitAsync" if f.contains('.') => {
                            for a in args.first().map(|a| dal_metinleri(*a, k)).unwrap_or_default() {
                                ekle("event", a);
                            }
                        }
                        "queue" | "queueAll" | "addJob" if f.contains('.') => {
                            for a in &args {
                                for ad in is_adlari(*a, k) {
                                    ekle("job", ad);
                                }
                            }
                        }
                        "add" | "addBulk" if f.to_lowercase().rsplit('.').nth(1).is_some_and(|r| r.contains("queue")) => {
                            if let Some(a) = args.first().and_then(|a| metin_sabiti(*a, k)) {
                                ekle("job", a);
                            }
                        }
                        _ => {}
                    }
                }
                // Kysely: `db.deleteFrom('asset')` tablo adıyla siler
                if parcalar.last().is_some_and(|s| *s == "deleteFrom") {
                    if let Some(t) = args.first().and_then(|a| metin_sabiti(*a, k)) {
                        if !d.silinen.contains(&t) {
                            d.silinen.push(t);
                        }
                    }
                }
                if let [.., "prisma", model, "delete" | "deleteMany"] = parcalar.as_slice() {
                    if !d.silinen.iter().any(|x| x == model) {
                        d.silinen.push(model.to_string());
                    }
                }
                if !f.contains('(') && d.cagrilar.len() < 64 {
                    let f: String = f.chars().filter(|c| !c.is_whitespace()).collect();
                    if !d.cagrilar.contains(&f) {
                        d.cagrilar.push(f);
                    }
                }
            }
            "ternary_expression" if d.dallar.len() < EN_FAZLA => {
                if let (Some(c), Some(e), Some(h)) = (n.child_by_field_name("condition"), n.child_by_field_name("consequence"), n.child_by_field_name("alternative")) {
                    let kosul = metin(c, k);
                    if basit_mi(e) && basit_mi(h) && kosul.len() <= 60 && !kosul.contains('\n') {
                        d.dallar.push(Dal {
                            kosul: kisa(&kosul),
                            evet: kisa(&metin(e, k)),
                            hayir: kisa(&metin(h, k)),
                            baglam: dal_baglami(n, k),
                            satir: n.start_position().row + 1,
                        });
                    }
                }
            }
            "pair" | "assignment_expression" if d.yumusak.is_none() => {
                let alan = n.child_by_field_name("key").or_else(|| n.child_by_field_name("left")).map(|a| metin(a, k)).unwrap_or_default();
                let deger = n.child_by_field_name("value").or_else(|| n.child_by_field_name("right")).map(|a| metin(a, k)).unwrap_or_default();
                if yumusak_yazma(&alan, &deger) {
                    d.yumusak = Some(n.start_position().row + 1);
                }
            }
            _ => {}
        }
        yigina_ekle(&mut yigin, n);
    }
    d
}

/// Python fonksiyon gövdesi (tree-sitter-python düğümü); iç fonksiyon ve sınıf tanımlarına girilmez
pub fn py_govde(govde: Node, k: &[u8]) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![govde];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "function_definition" | "class_definition" | "decorated_definition" if n.id() != govde.id() => continue,
            "raise_statement" if d.firlatmalar.len() < EN_FAZLA => {
                if let Some(c) = n.named_child(0).filter(|x| x.kind() == "call") {
                    let ad = c.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                    let mut durum = None;
                    for a in c.child_by_field_name("arguments").map(cocuklar).unwrap_or_default() {
                        match a.kind() {
                            "keyword_argument" if a.child_by_field_name("name").is_some_and(|x| metin(x, k) == "status_code") => {
                                durum = a.child_by_field_name("value").and_then(|v| kod_ifadesi(&metin(v, k)));
                            }
                            "keyword_argument" | "comment" => {}
                            _ if durum.is_none() => durum = kod_ifadesi(&metin(a, k)),
                            _ => {}
                        }
                    }
                    let kosul = kosul_bul(n, govde, k, true);
                    let sahiplik = kosul.as_deref().is_some_and(|x| sahiplik_mi(x, durum));
                    d.firlatmalar.push(Firlatma { durum, istisna: ad, kosul, satir: n.start_position().row + 1, sahiplik });
                }
            }
            "call" => {
                let f = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                // Arka plan işi: `background_tasks.add_task(gonder, ...)`, Celery `gonder.delay(...)` / `.apply_async(...)`
                let is = match f.rsplit_once('.') {
                    Some((_, "add_task")) => {
                        n.child_by_field_name("arguments").and_then(|a| a.named_child(0)).filter(|a| matches!(a.kind(), "identifier" | "attribute")).map(|a| metin(a, k))
                    }
                    Some((r, "delay" | "apply_async")) => Some(r.rsplit('.').next().unwrap_or(r).to_string()),
                    _ => None,
                };
                if let Some(is) = is.filter(|_| d.tetikler.len() < EN_FAZLA) {
                    let is = is.rsplit('.').next().unwrap_or(&is).to_string();
                    if !d.tetikler.iter().any(|(t, a, _)| t == "job" && *a == is) {
                        d.tetikler.push(("job".into(), is, n.start_position().row + 1));
                    }
                }
                if d.yumusak.is_none() && YUMUSAK_CAGRI.contains(&f.rsplit('.').next().unwrap_or("")) {
                    d.yumusak = Some(n.start_position().row + 1);
                }
                if !f.contains('(') && d.cagrilar.len() < 64 && !d.cagrilar.contains(&f) {
                    d.cagrilar.push(f);
                }
            }
            "conditional_expression" if d.dallar.len() < EN_FAZLA => {
                let c = cocuklar(n);
                if let [e, kos, h] = c.as_slice() {
                    let kosul = metin(*kos, k);
                    if basit_mi(*e) && basit_mi(*h) && kosul.len() <= 60 && !kosul.contains('\n') {
                        d.dallar.push(Dal {
                            kosul: kisa(&kosul),
                            evet: kisa(&metin(*e, k)),
                            hayir: kisa(&metin(*h, k)),
                            baglam: dal_baglami(n, k),
                            satir: n.start_position().row + 1,
                        });
                    }
                }
            }
            "assignment" | "keyword_argument" if d.yumusak.is_none() => {
                let alan = n.child_by_field_name("left").or_else(|| n.child_by_field_name("name")).map(|a| metin(a, k)).unwrap_or_default();
                let deger = n.child_by_field_name("right").or_else(|| n.child_by_field_name("value")).map(|a| metin(a, k)).unwrap_or_default();
                if yumusak_yazma(&alan, &deger) {
                    d.yumusak = Some(n.start_position().row + 1);
                }
            }
            _ => {}
        }
        yigina_ekle(&mut yigin, n);
    }
    d
}

/// Go durum adları (`http.StatusNotFound`, `fiber.StatusForbidden`) ve hazır hata değerleri (`echo.ErrNotFound`,
/// `fiber.ErrUnauthorized`) -> HTTP kodu
pub fn go_durum(ad: &str) -> Option<u16> {
    let ad = ad.rsplit('.').next().unwrap_or(ad).trim();
    let ad = ad.strip_prefix("Status").or_else(|| ad.strip_prefix("Err")).unwrap_or(ad);
    Some(match ad {
        "OK" => 200,
        "Created" => 201,
        "Accepted" => 202,
        "NoContent" => 204,
        "MovedPermanently" => 301,
        "Found" => 302,
        "SeeOther" => 303,
        "NotModified" => 304,
        "TemporaryRedirect" => 307,
        "PermanentRedirect" => 308,
        "BadRequest" => 400,
        "Unauthorized" => 401,
        "PaymentRequired" => 402,
        "Forbidden" => 403,
        "NotFound" => 404,
        "MethodNotAllowed" => 405,
        "NotAcceptable" => 406,
        "RequestTimeout" => 408,
        "Conflict" => 409,
        "Gone" => 410,
        "PreconditionFailed" => 412,
        "RequestEntityTooLarge" | "StatusRequestEntityTooLarge" => 413,
        "UnsupportedMediaType" => 415,
        "Teapot" => 418,
        "UnprocessableEntity" => 422,
        "Locked" => 423,
        "TooManyRequests" => 429,
        "InternalServerError" => 500,
        "NotImplemented" => 501,
        "BadGateway" => 502,
        "ServiceUnavailable" => 503,
        "GatewayTimeout" => 504,
        _ => return None,
    })
}

/// Go ifadesinden HTTP kodu: `404`, `http.StatusNotFound`, `fiber.StatusForbidden`
pub fn go_kod_ifadesi(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u16>() {
        return (100..600).contains(&n).then_some(n);
    }
    let son = s.rsplit('.').next().unwrap_or(s);
    son.starts_with("Status").then(|| go_durum(son)).flatten()
}

/// Go fonksiyon gövdesi (tree-sitter-go düğümü; iç fonksiyon değişmezleri dahil, ara katmanın döndürdüğü kapanış da
/// gövdedir):
/// - hata senaryoları: 4xx / 5xx kodla biten yanıtlar (`c.AbortWithStatusJSON(http.StatusUnauthorized, ...)`,
///   `c.JSON(404, ...)`, `http.Error(w, ..., http.StatusForbidden)`, `w.WriteHeader(403)`, `echo.NewHTTPError(400)`,
///   `fiber.NewError(...)`, `c.Status(404).JSON(...)`, `return echo.ErrNotFound`) ve argümanında `http.Status...` /
///   `fiber.Status...` 4xx-5xx sabiti geçen yardımcı çağrılar; koşulu en yakın `if`,
/// - yumuşak silme işareti (`DeletedAt` alanına yazma, `softDelete` çağrıları),
/// - olay ve iş: `go gonder(...)` (arka plan işi), `Publish("ad", ...)` / `Emit("ad")` (olay), `Enqueue(NewTask("ad"))`.
pub fn go_govde(govde: Node, k: &[u8]) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![govde];
    let ekle = |d: &mut GovdeDna, n: Node, durum: Option<u16>, istisna: String| {
        let satir = n.start_position().row + 1;
        if d.firlatmalar.len() >= EN_FAZLA || d.firlatmalar.iter().any(|f| f.satir == satir) {
            return;
        }
        let kosul = kosul_bul(n, govde, k, false);
        let sahiplik = kosul.as_deref().is_some_and(|c| sahiplik_mi(c, durum));
        d.firlatmalar.push(Firlatma { durum, istisna, kosul, satir, sahiplik });
    };
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "call_expression" => {
                let f: String = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
                let args: Vec<Node> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                let son = f.rsplit('.').next().unwrap_or("");
                let arg = |i: usize| args.get(i).map(|a| metin(*a, k)).unwrap_or_default();
                let kod = match son {
                    "AbortWithStatus"
                    | "AbortWithStatusJSON"
                    | "AbortWithError"
                    | "JSON"
                    | "IndentedJSON"
                    | "String"
                    | "NoContent"
                    | "Status"
                    | "SendStatus"
                    | "NewHTTPError"
                    | "NewError"
                    | "XML"
                    | "Redirect"
                        if f.contains('.') =>
                    {
                        go_kod_ifadesi(&arg(0))
                    }
                    "Error" if f == "http.Error" => go_kod_ifadesi(&arg(2)),
                    "WriteHeader" => go_kod_ifadesi(&arg(0)),
                    _ => None,
                };
                // Yardımcı (`respondError(w, http.StatusBadRequest, ...)`): argümanlarındaki adlı durum sabiti
                let kod = kod.or_else(|| {
                    args.iter()
                        .map(|a| metin(*a, k))
                        .filter(|a| a.starts_with("http.Status") || a.starts_with("fiber.Status") || a.starts_with("echo.Status"))
                        .find_map(|a| go_kod_ifadesi(&a))
                        .filter(|c| *c >= 400)
                });
                if let Some(kod) = kod.filter(|c| *c >= 400) {
                    ekle(&mut d, n, Some(kod), f.clone());
                }
                if d.yumusak.is_none() && YUMUSAK_CAGRI.contains(&son) {
                    d.yumusak = Some(n.start_position().row + 1);
                }
                if d.tetikler.len() < EN_FAZLA {
                    let mut tetik = |tur: &str, ad: String| {
                        if !ad.is_empty() && !d.tetikler.iter().any(|(t, a, _)| t == tur && *a == ad) {
                            d.tetikler.push((tur.to_string(), ad, n.start_position().row + 1));
                        }
                    };
                    match son {
                        "Publish" | "Emit" | "Dispatch" | "Fire" | "PublishEvent" if f.contains('.') => {
                            if let Some(a) = args.iter().take(2).find_map(|a| go_metin(*a, k)) {
                                tetik("event", a);
                            }
                        }
                        "NewTask" => {
                            if let Some(a) = args.first().and_then(|a| go_metin(*a, k)) {
                                tetik("job", a);
                            }
                        }
                        _ => {}
                    }
                }
                if d.cagrilar.len() < 64 && !f.contains('(') && !d.cagrilar.contains(&f) {
                    d.cagrilar.push(f);
                }
            }
            // `return echo.ErrNotFound` / `return fiber.ErrUnauthorized`
            "return_statement" => {
                for e in cocuklar(n).into_iter().flat_map(cocuklar) {
                    let t = metin(e, k);
                    if e.kind() == "selector_expression" && (t.starts_with("echo.Err") || t.starts_with("fiber.Err")) {
                        if let Some(kod) = go_durum(&t).filter(|c| *c >= 400) {
                            ekle(&mut d, n, Some(kod), t);
                        }
                    }
                }
            }
            "go_statement" if d.tetikler.len() < EN_FAZLA => {
                if let Some(c) = n.named_child(0).filter(|c| c.kind() == "call_expression") {
                    let f = c.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                    if !f.starts_with("func") && !d.tetikler.iter().any(|(t, a, _)| t == "job" && *a == f) {
                        let ad = f.rsplit('.').next().unwrap_or(&f).to_string();
                        d.tetikler.push(("job".into(), ad, n.start_position().row + 1));
                    }
                }
            }
            "assignment_statement" if d.yumusak.is_none() => {
                let alan = n.child_by_field_name("left").map(|a| metin(a, k)).unwrap_or_default();
                let deger = n.child_by_field_name("right").map(|a| metin(a, k)).unwrap_or_default();
                let alan_son = alan.rsplit('.').next().unwrap_or("");
                if (matches!(alan_son, "DeletedAt" | "IsDeleted" | "Deleted" | "TrashedAt") && (deger.contains("Now") || deger.contains("true") || deger.contains("Time")))
                    || yumusak_yazma(&alan, &deger)
                {
                    d.yumusak = Some(n.start_position().row + 1);
                }
            }
            _ => {}
        }
        yigina_ekle(&mut yigin, n);
    }
    d
}

/// Go metin sabiti: "x" ya da `x`
fn go_metin(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "interpreted_string_literal" => Some(metin(n, k).trim_matches('"').to_string()),
        "raw_string_literal" => Some(metin(n, k).trim_matches('`').to_string()),
        _ => None,
    }
}

/// Zincirdeki bir halka: gövdenin bulunduğu dosya, gövde, işlemden uzaklığı ve türü ("isleyici", "servis", "bagimlilik")
pub struct Halka<'a> {
    pub dosya: String,
    pub dna: &'a GovdeDna,
    pub derinlik: usize,
    pub tur: &'static str,
}

/// İşlemin zincirinden davranış özeti (kaskad ve geri alma `tamamla` ile eklenir)
pub fn ozet(halkalar: &[Halka]) -> Value {
    let mut hatalar = vec![];
    let mut sahiplik = vec![];
    let mut dallar = vec![];
    let mut yumusak = None;
    let mut silinen: BTreeSet<String> = BTreeSet::new();
    let mut tetikler: Vec<Value> = vec![];
    let mut gorulen = BTreeSet::new();
    for h in halkalar {
        let guven = if h.derinlik == 0 && h.tur == "isleyici" { "certain" } else { "inferred" };
        for f in &h.dna.firlatmalar {
            let kanit = format!("{}:{}", h.dosya, f.satir);
            if !gorulen.insert(kanit.clone()) || hatalar.len() >= EN_FAZLA {
                continue;
            }
            hatalar.push(json!({ "durum": f.durum, "istisna": f.istisna, "kosul": f.kosul, "kaynak": h.tur, "sahiplik": f.sahiplik, "kanit": kanit, "guven": guven }));
            if f.sahiplik {
                sahiplik.push(json!({ "kosul": f.kosul, "durum": f.durum, "kanit": kanit, "guven": guven }));
            }
        }
        for d in &h.dna.dallar {
            if dallar.len() < 8 {
                dallar.push(json!({ "kosul": d.kosul, "evet": d.evet, "hayir": d.hayir, "baglam": d.baglam, "kanit": format!("{}:{}", h.dosya, d.satir), "guven": guven }));
            }
        }
        if yumusak.is_none() {
            yumusak = h.dna.yumusak.map(|s| format!("{}:{s}", h.dosya));
        }
        silinen.extend(h.dna.silinen.iter().cloned());
        for (tur, ad, satir) in &h.dna.tetikler {
            if tetikler.len() < EN_FAZLA && !tetikler.iter().any(|t| t["tur"] == tur.as_str() && t["ad"] == ad.as_str()) {
                tetikler.push(json!({ "tur": tur, "ad": ad, "kanit": format!("{}:{satir}", h.dosya), "guven": "inferred" }));
            }
        }
    }
    json!({ "hatalar": hatalar, "sahiplik_denetimi": sahiplik, "dallar": dallar, "yumusak_silme": yumusak, "silinen": silinen, "tetikler": tetikler, "kapsam": "acik" })
}

/// Özette gösterilecek bir şey yok mu
pub fn bos(v: &Value) -> bool {
    ["hatalar", "dallar", "silinen", "tetikler"].iter().all(|a| v[a].as_array().is_none_or(|x| x.is_empty())) && v["yumusak_silme"].is_null()
}

/// İşlemlerin davranış özetini tamamlar: adaptörün zincir özetine (`bilgi.dna`) silme işlemlerinin kaskadla dolaylı
/// yazdıklarını (`dolayli_yazar`) ekler. Geri alınabilirlik (`geri_alinabilir`: evet | kismen | hayir | bilinmiyor)
/// dış etkilerle birlikte `dis::isle`'de verilir. Okuma işlemlerinde geri alma sorulmaz.
pub fn tamamla(kapilar: &[crate::adaptor::Kapi], bilgi: &Value) -> Value {
    let kaskadlar: Vec<Kaskad> =
        bilgi["kaskadlar"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_str()?.to_string(), k[1].as_str()?.to_string(), k[2].as_str()?.to_string()))).collect();
    let mut harita = bilgi["dna"].as_object().cloned().unwrap_or_default();
    for k in kapilar {
        if matches!(k.yontem.as_str(), "GET" | "HEAD" | "OPTIONS") {
            continue;
        }
        let yer = format!("{}:{}", k.dosya, k.satir);
        let mut v = harita.remove(&yer).unwrap_or_else(|| json!({ "hatalar": [], "sahiplik_denetimi": [], "dallar": [], "yumusak_silme": null, "kapsam": "acik" }));
        // Silinenler: veri bağındaki yazılan tablolar (silme işleminde) ve zincirde ORM ile silinen modeller
        let mut silinen: Vec<String> = v["silinen"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        if k.yontem == "DELETE" {
            silinen.extend(k.veri.as_ref().map(|v| v.yazar.clone()).unwrap_or_default());
        }
        if !silinen.is_empty() {
            let d = dolayli(&silinen, &kaskadlar);
            if !d.is_empty() {
                v["dolayli_yazar"] = json!(d);
            }
        }
        // Geri alınabilirlik kararı dış etkilerle birlikte verilir (dis::isle); burada yalnızca bilinmiyor
        if v["geri_alinabilir"].is_null() {
            v["geri_alinabilir"] = json!("bilinmiyor");
        }
        harita.insert(yer, v);
    }
    Value::Object(harita)
}

/// Dahili olay ve iş düğümleri (`bilgi.olaylar`: "event:X" / "job:X" -> {tur, ad, dinleyiciler, tetikler, silinen}):
/// dinleyicinin zincirinde silinen tablolardan kaskadla dolaylı yazılanlar eklenir. İşlemler (operations) değildir:
/// kartta kökteki `x-nodes` altında durur
pub fn olaylari_tamamla(bilgi: &Value) -> Value {
    let kaskadlar: Vec<Kaskad> =
        bilgi["kaskadlar"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_str()?.to_string(), k[1].as_str()?.to_string(), k[2].as_str()?.to_string()))).collect();
    let mut harita = bilgi["olaylar"].as_object().cloned().unwrap_or_default();
    for v in harita.values_mut() {
        let silinen: Vec<String> = v["silinen"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        let d = dolayli(&silinen, &kaskadlar);
        if !d.is_empty() {
            v["dolayli_yazar"] = json!(d);
        }
    }
    Value::Object(harita)
}

/// Aynı kaynakta geri yükleme ucu: yolu ya da fonksiyon adı restore / undelete / untrash / recover / undo içeren ve
/// silinen kaynağın adını (`assets`) yolunda taşıyan işlem. Yalnızca kaynağın kendisini silen uçlar için aranır
/// (`DELETE /assets`, `DELETE /assets/:id`); alt ilişkiyi kaldıran uçlar (`DELETE /albums/:id/assets`) sayılmaz.
/// Geri yükleme sözcüğü yolda tek başına bir parça (`/trash/restore/assets`, `/users/:id/restore`) ya da fonksiyon adının
/// başı (`restoreUser`) olmalı: `/database-backups/start-restore` (yedekten geri dönme) silmenin tersi değildir
pub(crate) fn geri_yukleme_ucu<'k>(k: &crate::adaptor::Kapi, kapilar: &'k [crate::adaptor::Kapi]) -> Option<&'k crate::adaptor::Kapi> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let ters = D.get_or_init(|| regex::Regex::new(r"(?i)^(restore|undelete|untrash|recover|undo|unarchive|reactivate|revert|geri-al|geri-yukle)").unwrap());
    let ters_yol = |y: &str| {
        y.split('/').any(|p| {
            matches!(p.to_lowercase().as_str(), "restore" | "undelete" | "untrash" | "recover" | "undo" | "unarchive" | "reactivate" | "revert" | "geri-al" | "geri-yukle")
        })
    };
    let parcalar = |y: &str| -> Vec<String> {
        y.split('/')
            .filter(|p| p.len() > 2 && !p.starts_with(':') && !p.starts_with('{') && !matches!(*p, "api" | "v1" | "v2" | "v3" | "me" | "admin"))
            .map(|p| p.to_lowercase().trim_end_matches('s').to_string())
            .collect()
    };
    let benim = parcalar(&k.yol);
    if benim.len() != 1 {
        return None;
    }
    kapilar
        .iter()
        .filter(|t| t.id != k.id && t.yontem != "GET" && (ters_yol(&t.yol) || ters.is_match(&t.fonksiyon)))
        // Geri yüklenen kaynak yoldaki son anlamlı parçadır: `/trash/restore/assets` varlığı geri yükler, ama
        // `/books/:slug/page/:p/revisions/:id/restore` bir sürümü geri yükler, kitabı değil
        .find(|t| parcalar(&t.yol).into_iter().rfind(|p| !ters.is_match(p)).is_some_and(|p| benim.contains(&p)))
}

/// Olay adının olası metinleri: 'X' ya da `kosul ? 'X' : 'Y'` dallarındaki metinler
fn dal_metinleri(n: Node, k: &[u8]) -> Vec<String> {
    if n.kind() == "ternary_expression" {
        return ["consequence", "alternative"].iter().filter_map(|a| n.child_by_field_name(a)).flat_map(|x| dal_metinleri(x, k)).collect();
    }
    metin_sabiti(n, k).into_iter().collect()
}

/// İş adları: argümandaki `{ name: JobName.X }` / `{ name: 'x' }` nesneleri (iç içe dizi ve ok fonksiyonları dahil);
/// enum üyesi son parçasıyla (`JobName.AssetDelete` -> "AssetDelete")
fn is_adlari(n: Node, k: &[u8]) -> Vec<String> {
    let mut v = vec![];
    let mut yigin = vec![n];
    while let Some(x) = yigin.pop() {
        if x.kind() == "pair" && x.child_by_field_name("key").is_some_and(|a| metin(a, k) == "name") {
            if let Some(d) = x.child_by_field_name("value") {
                if let Some(a) = is_adi(d, k) {
                    if !v.contains(&a) {
                        v.push(a);
                    }
                }
            }
            continue;
        }
        if v.len() < 8 {
            yigin.extend(cocuklar(x));
        }
    }
    v
}

/// Olay ya da iş adı ifadesi: metin ya da enum üyesi (son parça)
fn is_adi(d: Node, k: &[u8]) -> Option<String> {
    match d.kind() {
        "member_expression" => d.child_by_field_name("property").map(|p| metin(p, k)),
        "identifier" => Some(metin(d, k)),
        _ => metin_sabiti(d, k),
    }
}

/// Dinleyici dekoratörü: `@OnEvent('x')`, `@OnEvent({ name: 'x' })`, `@OnJob({ name: JobName.X })`, `@Process('x')`,
/// `@EventPattern('x')`, `@MessagePattern('x')`. Dönen: (tür, ad)
pub fn dinleyici(dekorator: &str, argumanlar: &[String]) -> Option<(&'static str, String)> {
    let tur = match dekorator {
        "OnEvent" | "EventPattern" | "MessagePattern" => "event",
        "OnJob" | "Process" | "OnQueueEvent" => "job",
        _ => return None,
    };
    let a = argumanlar.first()?.trim();
    let ham = if a.starts_with('{') {
        let i = a.find("name")?;
        let kalan = a[i + 4..].trim_start().strip_prefix(':')?.trim_start();
        kalan.split([',', '}']).next()?.trim().to_string()
    } else {
        a.to_string()
    };
    let ad = ham.trim_matches(['\'', '"', '`']);
    let ad = if ham.starts_with(['\'', '"', '`']) { ad } else { ad.rsplit('.').next().unwrap_or(ad) };
    (!ad.is_empty() && ad.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))).then(|| (tur, ad.to_string()))
}

/// Metin sabiti: 'x', "x" ya da değişkensiz `x`
fn metin_sabiti(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "string" => Some(metin(n, k).trim_matches(['\'', '"']).to_string()),
        "template_string" if cocuklar(n).iter().all(|c| c.kind() != "template_substitution") => Some(metin(n, k).trim_matches('`').to_string()),
        _ => None,
    }
}

/// Göç dosyaları: `.sql` dosyaları ve yolunda `migration` ya da `alembic` geçen TS / JS / Python dosyaları (gitignore'a uyarak,
/// bağımlılık ve derleme klasörleri hariç, 4 MiB sınırı). Dönen: (mutlak yol, göreli yol, metin), göreli yola göre sıralı
pub fn goc_dosyalari(kok: &std::path::Path) -> Vec<(std::path::PathBuf, String, String)> {
    let mut v = vec![];
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            continue;
        }
        let goreli = crate::guvenli::goreli(kok, p);
        if goreli.split('/').any(|c| matches!(c, "node_modules" | "dist" | "build" | ".venv" | "venv" | "site-packages" | "target" | "__pycache__")) {
            continue;
        }
        let uzanti = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let alt = goreli.to_ascii_lowercase();
        let goc = (alt.contains("migration") || alt.contains("alembic")) && matches!(uzanti, "ts" | "js" | "mjs" | "cjs" | "py" | "go");
        if uzanti != "sql" && !goc {
            continue;
        }
        if let Some(m) = crate::guvenli::oku(p) {
            v.push((p.to_path_buf(), goreli, m));
        }
    }
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

/// Göç dosyasındaki bir olay: Ok((ebeveyn, çocuk)) kaskad, Err((eski, yeni)) yeniden adlandırma
type GocOlayi = Result<(String, String), (String, String)>;

/// Göç dosyalarındaki (ham SQL, Kysely / TypeORM göçlerindeki SQL metni, Alembic) kaskad silmeler:
/// - `ALTER TABLE "cocuk" ADD [CONSTRAINT x] FOREIGN KEY (...) REFERENCES "ebeveyn" (...) ... ON DELETE CASCADE`,
/// - `CREATE TABLE "cocuk" ( ... REFERENCES "ebeveyn" ... ON DELETE CASCADE ... )`,
/// - Alembic `op.create_foreign_key(ad, 'cocuk', 'ebeveyn', ..., ondelete='CASCADE')` ve
///   `op.create_table('cocuk', ... ForeignKey('ebeveyn.id', ondelete='CASCADE'))`,
/// - Django `migrations.CreateModel(name="Cocuk", fields=[... models.ForeignKey(on_delete=...CASCADE, to="app.ebeveyn")])`
///   ve `migrations.AddField(model_name="cocuk", field=models.ForeignKey(...))` (model adlarıyla).
///
/// Tablo yeniden adlandırmaları (`ALTER TABLE "x" RENAME TO "y"`) sırayla izlenir: kaskad güncel adla yazılır.
/// `dosyalar`: (göreli yol, metin), göç sırasıyla (dosya adına göre sıralı)
pub fn goc_kaskadlari(dosyalar: &[(String, String)]) -> Vec<Kaskad> {
    static D: std::sync::OnceLock<[regex::Regex; 7]> = std::sync::OnceLock::new();
    let [alter, create, referans, yeniden, alembic_fk, alembic_tablo, alembic_ref] = D.get_or_init(|| {
        let ad = r#"[`"\[]?(?:\w+[`"\]]?\.[`"\[]?)?(\w+)[`"\]]?"#;
        [
            regex::Regex::new(&format!(r"(?is)ALTER\s+TABLE\s+(?:ONLY\s+)?(?:IF\s+EXISTS\s+)?{ad}\s+ADD\s+(?:CONSTRAINT\s+\S+\s+)?FOREIGN\s+KEY\s*\([^)]*\)\s*REFERENCES\s+{ad}[^;`]*?ON\s+DELETE\s+CASCADE")).unwrap(),
            regex::Regex::new(&format!(r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?{ad}\s*\(")).unwrap(),
            regex::Regex::new(&format!(r"(?is)REFERENCES\s+{ad}\s*(?:\([^)]*\))?[^,;]*?ON\s+DELETE\s+CASCADE")).unwrap(),
            regex::Regex::new(&format!(r"(?is)ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?{ad}\s+RENAME\s+TO\s+{ad}")).unwrap(),
            regex::Regex::new(r#"create_foreign_key\(\s*[^,]+,\s*['"](\w+)['"]\s*,\s*['"](\w+)['"][^)]*ondelete\s*=\s*['"]CASCADE['"]"#).unwrap(),
            regex::Regex::new(r#"create_table\(\s*['"](\w+)['"]"#).unwrap(),
            regex::Regex::new(r#"ForeignKey(?:Constraint)?\(\s*(?:\[[^\]]*\]\s*,\s*\[\s*)?['"](\w+)\.\w+['"][^)]*ondelete\s*=\s*['"]CASCADE['"]"#).unwrap(),
        ]
    });
    // Kaskadlar (ebeveyn, çocuk, kanıt, sıra) ve yeniden adlandırmalar (eski, yeni, sıra)
    let mut ham: Vec<(String, String, String, usize)> = vec![];
    let mut adlar: Vec<(String, String, usize)> = vec![];
    let mut sira = 0usize;
    for (yol, metin) in dosyalar {
        let buyuk = metin.to_ascii_uppercase();
        if !buyuk.contains("CASCADE") && !buyuk.contains("RENAME") {
            continue;
        }
        let satir = |i: usize| metin[..i].matches('\n').count() + 1;
        let mut olaylar: Vec<(usize, GocOlayi)> = vec![];
        for c in alter.captures_iter(metin) {
            olaylar.push((c.get(0).map_or(0, |m| m.start()), Ok((c[2].to_string(), c[1].to_string()))));
        }
        for c in create.captures_iter(metin) {
            let bas = c.get(0).map_or(0, |m| m.end());
            // Tablo gövdesi: eşleşen kapanış parantezine kadar
            let mut derinlik = 1i32;
            let mut son = metin.len();
            for (i, ch) in metin[bas..].char_indices() {
                match ch {
                    '(' => derinlik += 1,
                    ')' => {
                        derinlik -= 1;
                        if derinlik == 0 {
                            son = bas + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            for r in referans.captures_iter(&metin[bas..son]) {
                olaylar.push((bas + r.get(0).map_or(0, |m| m.start()), Ok((r[1].to_string(), c[1].to_string()))));
            }
        }
        for c in alembic_fk.captures_iter(metin) {
            olaylar.push((c.get(0).map_or(0, |m| m.start()), Ok((c[2].to_string(), c[1].to_string()))));
        }
        let tablolar: Vec<(usize, String)> = alembic_tablo.captures_iter(metin).map(|c| (c.get(0).map_or(0, |m| m.start()), c[1].to_string())).collect();
        for c in alembic_ref.captures_iter(metin) {
            let i = c.get(0).map_or(0, |m| m.start());
            if let Some((_, t)) = tablolar.iter().rev().find(|(j, _)| *j < i) {
                olaylar.push((i, Ok((c[1].to_string(), t.clone()))));
            }
        }
        for c in yeniden.captures_iter(metin) {
            olaylar.push((c.get(0).map_or(0, |m| m.start()), Err((c[1].to_string(), c[2].to_string()))));
        }
        olaylar.extend(django_goc_kaskadlari(metin).into_iter().map(|(i, e, c)| (i, Ok((e, c)))));
        olaylar.sort_by_key(|o| o.0);
        for (i, o) in olaylar {
            sira += 1;
            match o {
                Ok((e, c)) => ham.push((e, c, format!("{yol}:{}", satir(i)), sira)),
                Err((eski, yeni)) => adlar.push((eski, yeni, sira)),
            }
        }
    }
    // Ad, kaskadın tanımlandığı andan sonraki yeniden adlandırmalardan geçirilir
    let guncel = |ad: &str, sonra: usize| -> String {
        let mut a = ad.to_string();
        for (eski, yeni, s) in &adlar {
            if *s > sonra && eski.eq_ignore_ascii_case(&a) {
                a = yeni.clone();
            }
        }
        a
    };
    let mut v: Vec<Kaskad> = vec![];
    for (e, c, kanit, s) in ham {
        let (e, c) = (guncel(&e, s), guncel(&c, s));
        if !e.eq_ignore_ascii_case(&c) && !v.iter().any(|(a, b, _)| a.eq_ignore_ascii_case(&e) && b.eq_ignore_ascii_case(&c)) {
            v.push((e, c, kanit));
        }
    }
    v
}

/// Django göç dosyasındaki kaskadlar: (konum, ebeveyn, çocuk). Çocuk, ilişki alanından önceki son `CreateModel(name=...)`
/// ya da `AddField(model_name=...)`'dır; ebeveyn `to="uygulama.model"`
fn django_goc_kaskadlari(metin: &str) -> Vec<(usize, String, String)> {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [model, iliski, hedef] = D.get_or_init(|| {
        [
            regex::Regex::new(r#"migrations\.(?:CreateModel\(\s*name|AddField\(\s*model_name)\s*=\s*['"](\w+)['"]"#).unwrap(),
            regex::Regex::new(r"\b(?:models\.)?(?:ForeignKey|OneToOneField)\(").unwrap(),
            regex::Regex::new(r#"\bto\s*=\s*['"](?:\w+\.)?(\w+)['"]"#).unwrap(),
        ]
    });
    let modeller: Vec<(usize, String)> = model.captures_iter(metin).map(|c| (c.get(0).map_or(0, |m| m.start()), c[1].to_string())).collect();
    let mut v = vec![];
    for m in iliski.find_iter(metin) {
        // Çağrının sonu: dengeli parantez
        let (mut derinlik, mut son) = (1i32, metin.len());
        for (i, ch) in metin[m.end()..].char_indices() {
            match ch {
                '(' => derinlik += 1,
                ')' => {
                    derinlik -= 1;
                    if derinlik == 0 {
                        son = m.end() + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let govde = &metin[m.end()..son];
        let silme = govde
            .split("on_delete")
            .nth(1)
            .map(|x| x.trim_start().trim_start_matches('=').trim_start().split([',', ')']).next().unwrap_or("").trim().to_string())
            .unwrap_or_default();
        if !silme.ends_with("CASCADE") {
            continue;
        }
        let (Some(h), Some((_, cocuk))) = (hedef.captures(govde), modeller.iter().rev().find(|(i, _)| *i < m.start())) else { continue };
        v.push((m.start(), h[1].to_string(), cocuk.clone()));
    }
    v
}

/// ORM şemasından kaskad: (ebeveyn model, çocuk model, kanıt "dosya:satır")
pub type Kaskad = (String, String, String);

/// Prisma şemasından kaskadlar: `model Cocuk { ebeveyn Ebeveyn @relation(..., onDelete: Cascade) }`
pub fn prisma_kaskadlari(dosya: &str, metin: &str) -> Vec<Kaskad> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let alan = D.get_or_init(|| regex::Regex::new(r"^\s*\w+\s+([A-Z]\w*)\??\s+@relation\(.*onDelete:\s*Cascade").unwrap());
    let mut v = vec![];
    let mut model: Option<String> = None;
    for (i, s) in metin.lines().enumerate() {
        let t = s.trim();
        if let Some(r) = t.strip_prefix("model ") {
            model = r.split_whitespace().next().map(String::from);
        } else if t.starts_with('}') {
            model = None;
        } else if let (Some(m), Some(c)) = (&model, alan.captures(s)) {
            v.push((c[1].to_string(), m.clone(), format!("{dosya}:{}", i + 1)));
        }
    }
    v
}

/// Varlık sınıfından kaskad: TypeORM `@ManyToOne(() => Ebeveyn, ..., { onDelete: 'CASCADE' })`, `@ForeignKeyColumn(() => X, { onDelete: 'CASCADE' })`
pub fn typeorm_kaskadi(sinif: &str, dekorator: &str, argumanlar: &[String]) -> Option<String> {
    if !matches!(dekorator, "ManyToOne" | "OneToOne" | "ForeignKeyColumn") {
        return None;
    }
    // Yalnızca silme kuralı: `onUpdate: 'CASCADE'` ile `onDelete: 'SET NULL'` birlikte olabilir
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let silme = D.get_or_init(|| regex::Regex::new(r#"(?i)onDelete\s*:\s*['"`]CASCADE['"`]"#).unwrap());
    if !silme.is_match(&argumanlar.join(" ")) {
        return None;
    }
    let ebeveyn: String = argumanlar.first()?.rsplit("=>").next()?.trim().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!ebeveyn.is_empty() && ebeveyn != sinif).then_some(ebeveyn)
}

/// SQLModel / SQLAlchemy alanından kaskad. Dönen: (ebeveyn, çocuk)
/// - çocukta `Field(foreign_key="user.id", ondelete="CASCADE")` / `ForeignKey("user.id", ondelete="CASCADE")`: tablo adı
///   ebeveyn modelin adıyla eşlenir (`tablolar`: küçük harfli tablo adı -> model)
/// - ebeveynde `Relationship(cascade_delete=True)` / `relationship(cascade="all, delete...")`: tipteki model çocuktur
pub fn python_kaskadi(model: &str, tip: Option<&str>, varsayilan: &str, tablolar: &BTreeMap<String, String>) -> Option<(String, String)> {
    let v: String = varsayilan.chars().filter(|c| !c.is_whitespace()).collect();
    if (v.contains("ondelete=\"CASCADE\"") || v.contains("ondelete='CASCADE'") || v.to_lowercase().contains("ondelete=\"cascade\""))
        && (v.contains("foreign_key=") || v.contains("ForeignKey("))
    {
        let tablo = v.split(['"', '\'']).find(|p| p.contains('.') && !p.contains('=')).and_then(|p| p.split('.').next())?.to_lowercase();
        let ebeveyn = tablolar.get(&tablo).cloned()?;
        return (ebeveyn != model).then(|| (ebeveyn, model.to_string()));
    }
    let iliski = v.starts_with("Relationship(") || v.starts_with("relationship(");
    if iliski && (v.contains("cascade_delete=True") || (v.contains("cascade=") && v.contains("delete"))) {
        let tip = tip.unwrap_or("").to_string() + " " + varsayilan;
        let cocuk = tablolar.values().find(|m| *m != model && regex_kelime(&tip, m))?.clone();
        return Some((model.to_string(), cocuk));
    }
    None
}

fn regex_kelime(metin: &str, kelime: &str) -> bool {
    metin.match_indices(kelime).any(|(i, _)| {
        let once = metin[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let sonra = metin[i + kelime.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
        once && sonra
    })
}

/// Silme işleminin dolaylı yazdıkları: yazdığı tablolardan kaskadla ulaşılanlar (geçişli)
pub fn dolayli(yazar: &[String], kaskadlar: &[Kaskad]) -> Vec<Value> {
    let mut v = vec![];
    // Model adları büyük-küçük harf duyarsız eşlenir (Prisma istemci erişimcisi `makale` <-> model `Makale`)
    let mut gorulen: BTreeSet<String> = yazar.iter().map(|y| y.to_lowercase()).collect();
    let mut kuyruk: Vec<String> = gorulen.iter().cloned().collect();
    while let Some(e) = kuyruk.pop() {
        for (eb, cocuk, kanit) in kaskadlar.iter().filter(|(eb, _, _)| eb.to_lowercase() == e) {
            if gorulen.insert(cocuk.to_lowercase()) {
                v.push(json!({ "tablo": cocuk, "neden": format!("{eb} silinince kaskad"), "kanit": kanit, "guven": "inferred" }));
                kuyruk.push(cocuk.to_lowercase());
            }
        }
    }
    v
}
