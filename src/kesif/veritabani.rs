//! SQLite dosyasının şeması, SQLite kütüphanesi olmadan: dosya biçimi doğrudan okunur (yeni bağımlılık yok).
//! - Yalnızca okunur; dosya açılmaz, kilitlenmez, değiştirilmez. Hiçbir SQL çalıştırılmaz: tetikleyici, görünüm ve
//!   CHECK metinleri yalnızca metin olarak kanıta girer.
//! - Okunan: başlık (sayfa boyutu, metin kodlaması, WAL), `sqlite_master` tablosunun b-ağacı (sayfa 1'den başlayarak
//!   iç ve yaprak tablo sayfaları, taşma sayfaları) ve kayıt biçimi. Tablo, sütun, CHECK, FK, indeks, tetikleyici ve
//!   görünüm adları CREATE metinlerinden çıkarılır.
//! - Bozuk ya da kasıtlı kötü dosya: her konum ve uzunluk denetlenir; döngü, aşırı derinlik ve aşırı büyüklük
//!   sınırlanır; panik yerine `Err` ya da `sorunlar`.
//! - Kanıt türü DATABASE_SCHEMA. Dosyadan okunan şema yapıtın kendisidir ama kaynak kodu (migration) değildir:
//!   güven `observed` (ikili metadata ile aynı düzey).
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Gezilecek en fazla şema sayfası ve tek bir şema satırının en büyük boyu
const EN_FAZLA_SAYFA: usize = 20_000;
const EN_BUYUK_KAYIT: u64 = 4 << 20;
/// Kanıta girecek SQL metninin en büyük boyu (karakter)
const EN_UZUN_METIN: usize = 4000;

#[derive(Debug, Default)]
pub(crate) struct Sema {
    pub sayfa_boyutu: u32,
    pub kodlama: &'static str,
    pub wal: bool,
    /// `sqlite_master` satırları: (tür, ad, tablo, sql)
    pub satirlar: Vec<(String, String, String, Option<String>)>,
    pub sorunlar: Vec<String>,
}

impl Sema {
    /// Dosya düzeyindeki biçim bilgisi
    pub fn dosya(&self) -> Value {
        json!({ "bicim": "sqlite3", "sayfa_boyutu": self.sayfa_boyutu, "kodlama": self.kodlama, "wal": self.wal, "satir_sayisi": self.satirlar.len() })
    }
}

/// Sayfa okuyucu: dosyadan yalnızca istenen sayfa okunur
struct Dosya<R: Read + Seek> {
    f: R,
    uzunluk: u64,
    boyut: u32,
    kullanilan: u32,
}

impl<R: Read + Seek> Dosya<R> {
    fn sayfa(&mut self, n: u32) -> Option<Vec<u8>> {
        let bas = u64::from(n.checked_sub(1)?) * u64::from(self.boyut);
        if n == 0 || bas + u64::from(self.boyut) > self.uzunluk {
            return None;
        }
        self.f.seek(SeekFrom::Start(bas)).ok()?;
        let mut v = vec![0; self.boyut as usize];
        self.f.read_exact(&mut v).ok()?;
        Some(v)
    }
}

fn u16be(v: &[u8], o: usize) -> Option<usize> {
    v.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
}
fn u32be(v: &[u8], o: usize) -> Option<u32> {
    v.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// SQLite değişken uzunluklu tamsayısı: (değer, uzunluk)
fn varint(v: &[u8], o: usize) -> Option<(u64, usize)> {
    let mut x: u64 = 0;
    for i in 0..9 {
        let b = *v.get(o + i)?;
        if i == 8 {
            return Some(((x << 8) | u64::from(b), 9));
        }
        x = (x << 7) | u64::from(b & 0x7f);
        if b & 0x80 == 0 {
            return Some((x, i + 1));
        }
    }
    None
}

/// Dosyanın şemasını okur
pub(crate) fn oku(yol: &Path) -> Result<Sema, String> {
    let f = std::fs::File::open(yol).map_err(|e| format!("açılamadı: {e}"))?;
    let uzunluk = f.metadata().map_err(|e| e.to_string())?.len();
    let mut s = oku_kaynak(f, uzunluk)?;
    let wal = yol.with_file_name(format!("{}-wal", yol.file_name().and_then(|a| a.to_str()).unwrap_or("")));
    if std::fs::metadata(&wal).is_ok_and(|m| m.len() > 32) {
        s.sorunlar.push("yanında WAL dosyası var: son şema değişiklikleri okunmamış olabilir".into());
    }
    Ok(s)
}

/// Okunabilir herhangi bir kaynaktan (dosya ya da bellek) şema
pub(crate) fn oku_kaynak<R: Read + Seek>(mut f: R, uzunluk: u64) -> Result<Sema, String> {
    let mut baslik = [0u8; 100];
    f.read_exact(&mut baslik).map_err(|_| "SQLite başlığı kısa".to_string())?;
    if &baslik[..16] != b"SQLite format 3\0" {
        return Err("SQLite dosyası değil (başlık imzası yok)".into());
    }
    let boyut = match u16be(&baslik, 16) {
        Some(1) => 65536,
        Some(b) if (512..=32768).contains(&b) && b.is_power_of_two() => b as u32,
        _ => return Err("sayfa boyutu geçersiz".into()),
    };
    let ayrilan = u32::from(baslik[20]);
    let kullanilan = boyut.checked_sub(ayrilan).filter(|k| *k >= 480).ok_or("kullanılabilir sayfa alanı geçersiz")?;
    let kodlama = match u32be(&baslik, 56) {
        Some(2) => "utf-16le",
        Some(3) => "utf-16be",
        _ => "utf-8",
    };
    let mut s = Sema { sayfa_boyutu: boyut, kodlama, wal: baslik[18] == 2 || baslik[19] == 2, ..Sema::default() };
    let mut d = Dosya { f, uzunluk, boyut, kullanilan };
    // b-ağacı gezisi (yığınla; ziyaret edilen sayfa yeniden gezilmez: döngüye karşı)
    let mut yigin = vec![1u32];
    let mut gorulen = HashSet::new();
    while let Some(n) = yigin.pop() {
        if !gorulen.insert(n) {
            s.sorunlar.push(format!("sayfa {n} birden fazla kez gösteriliyor (döngü)"));
            continue;
        }
        if gorulen.len() > EN_FAZLA_SAYFA {
            s.sorunlar.push(format!("şema {EN_FAZLA_SAYFA} sayfayı aşıyor: kalanı okunmadı"));
            break;
        }
        let Some(v) = d.sayfa(n) else {
            s.sorunlar.push(format!("sayfa {n} dosyanın dışında"));
            continue;
        };
        let h = if n == 1 { 100 } else { 0 };
        let tur = v[h];
        let Some(hucre_sayisi) = u16be(&v, h + 3) else { continue };
        let (baslik_boyu, ic) = match tur {
            0x0d => (8, false),
            0x05 => (12, true),
            x => {
                s.sorunlar.push(format!("sayfa {n}: şema ağacında beklenmeyen sayfa türü {x}"));
                continue;
            }
        };
        if hucre_sayisi > d.kullanilan as usize / 2 {
            s.sorunlar.push(format!("sayfa {n}: hücre sayısı geçersiz"));
            continue;
        }
        if ic {
            if let Some(sag) = u32be(&v, h + 8) {
                yigin.push(sag);
            }
        }
        for i in 0..hucre_sayisi {
            let Some(o) = u16be(&v, h + baslik_boyu + 2 * i).filter(|o| *o < d.kullanilan as usize) else {
                s.sorunlar.push(format!("sayfa {n}: hücre konumu geçersiz"));
                continue;
            };
            if ic {
                if let Some(c) = u32be(&v, o) {
                    yigin.push(c);
                }
                continue;
            }
            match hucre(&mut d, &v, o).and_then(|y| satir(&y, kodlama)) {
                Some(r) => s.satirlar.push(r),
                None => s.sorunlar.push(format!("sayfa {n}: okunamayan şema satırı")),
            }
        }
    }
    Ok(s)
}

/// Yaprak tablo hücresinin yükü (taşma sayfaları dahil)
fn hucre<R: Read + Seek>(d: &mut Dosya<R>, v: &[u8], o: usize) -> Option<Vec<u8>> {
    let (yuk, a) = varint(v, o)?;
    let (_, b) = varint(v, o + a)?;
    if yuk > EN_BUYUK_KAYIT {
        return None;
    }
    let bas = o + a + b;
    let u = u64::from(d.kullanilan);
    let x = u - 35;
    let yerel = if yuk <= x {
        yuk
    } else {
        let m = ((u - 12) * 32 / 255) - 23;
        let k = m + ((yuk - m) % (u - 4));
        if k <= x {
            k
        } else {
            m
        }
    } as usize;
    let mut cikti = v.get(bas..bas.checked_add(yerel)?.min(d.kullanilan as usize))?.to_vec();
    if cikti.len() != yerel {
        return None;
    }
    let mut sonraki = if (yuk as usize) > yerel { u32be(v, bas + yerel)? } else { 0 };
    let mut gorulen = HashSet::new();
    while (cikti.len() as u64) < yuk {
        if sonraki == 0 || !gorulen.insert(sonraki) {
            return None;
        }
        let t = d.sayfa(sonraki)?;
        sonraki = u32be(&t, 0)?;
        let kalan = (yuk as usize) - cikti.len();
        cikti.extend_from_slice(t.get(4..(4 + kalan).min(d.kullanilan as usize))?);
    }
    Some(cikti)
}

/// Kayıt biçimi: `sqlite_master` satırı (type, name, tbl_name, rootpage, sql)
fn satir(y: &[u8], kodlama: &str) -> Option<(String, String, String, Option<String>)> {
    let (baslik, mut o) = varint(y, 0)?;
    let baslik = usize::try_from(baslik).ok().filter(|b| *b <= y.len())?;
    let mut turler = Vec::new();
    while o < baslik && turler.len() < 16 {
        let (t, n) = varint(y, o)?;
        turler.push(t);
        o += n;
    }
    let mut veri = baslik;
    let mut degerler: Vec<Option<String>> = Vec::new();
    for t in turler.iter().take(5) {
        let boy = match *t {
            0 | 8 | 9 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 6,
            6 | 7 => 8,
            t if t >= 12 => usize::try_from((t - 12) / 2).ok()?,
            _ => return None,
        };
        let parca = y.get(veri..veri.checked_add(boy)?)?;
        degerler.push((*t >= 13 && t % 2 == 1).then(|| metin(parca, kodlama)));
        veri += boy;
    }
    let mut d = degerler.into_iter();
    let tur = d.next()??;
    let ad = d.next()??;
    let tablo = d.next()?.unwrap_or_default();
    let _kok = d.next();
    let sql = d.next().flatten();
    Some((tur, ad, tablo, sql))
}

fn metin(b: &[u8], kodlama: &str) -> String {
    match kodlama {
        "utf-16le" => String::from_utf16_lossy(&b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect::<Vec<_>>()),
        "utf-16be" => String::from_utf16_lossy(&b.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect::<Vec<_>>()),
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// CREATE metinlerinin çözümlemesi (çalıştırılmaz; yalnızca ad ve kısıt metni)

/// Belirteç: tür ve kaynak metindeki konumu
#[derive(Debug, Clone, PartialEq)]
enum Bt {
    /// Ad ya da anahtar kelime (tırnaksız) ve tırnaklı ad ("x", `x`, [x])
    Ad(String, bool),
    Dize,
    Sayi,
    Isaret(char),
}

fn belirtecler(s: &str) -> Vec<(Bt, usize, usize)> {
    let b: Vec<char> = s.chars().collect();
    let bayt: Vec<usize> = s.char_indices().map(|(i, _)| i).chain(std::iter::once(s.len())).collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() && v.len() < 100_000 {
        let c = b[i];
        let bas = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '-' && b.get(i + 1) == Some(&'-') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }
        let kapanis = match c {
            '"' => Some('"'),
            '`' => Some('`'),
            '[' => Some(']'),
            '\'' => Some('\''),
            _ => None,
        };
        if let Some(k) = kapanis {
            i += 1;
            let mut ic = String::new();
            while i < b.len() {
                if b[i] == k {
                    if k != ']' && b.get(i + 1) == Some(&k) {
                        ic.push(k);
                        i += 2;
                        continue;
                    }
                    break;
                }
                ic.push(b[i]);
                i += 1;
            }
            i = (i + 1).min(b.len());
            v.push((if c == '\'' { Bt::Dize } else { Bt::Ad(ic, true) }, bayt[bas], bayt[i]));
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '$' {
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_' || b[i] == '$') {
                i += 1;
            }
            let k: String = b[bas..i].iter().collect();
            let tur = if k.chars().next().is_some_and(|c| c.is_ascii_digit()) { Bt::Sayi } else { Bt::Ad(k, false) };
            v.push((tur, bayt[bas], bayt[i]));
            continue;
        }
        v.push((Bt::Isaret(c), bayt[bas], bayt[bas + 1]));
        i += 1;
    }
    v
}

/// Tırnaksız anahtar kelime mi
fn kelime(t: Option<&(Bt, usize, usize)>, k: &str) -> bool {
    matches!(t, Some((Bt::Ad(a, false), _, _)) if a.eq_ignore_ascii_case(k))
}

fn ad(t: Option<&(Bt, usize, usize)>) -> Option<String> {
    match t {
        Some((Bt::Ad(a, _), _, _)) => Some(a.clone()),
        _ => None,
    }
}

/// `(`'den eşleşen `)`'ye kadar: (iç belirteçler aralığı, kapanıştan sonraki konum)
fn parantez(t: &[(Bt, usize, usize)], bas: usize) -> Option<(usize, usize, usize)> {
    if t.get(bas)?.0 != Bt::Isaret('(') {
        return None;
    }
    let mut derinlik = 0;
    for (i, (b, _, _)) in t.iter().enumerate().skip(bas) {
        match b {
            Bt::Isaret('(') => derinlik += 1,
            Bt::Isaret(')') => {
                derinlik -= 1;
                if derinlik == 0 {
                    return Some((bas + 1, i, i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Parantez içindeki metin (kaynaktan aynen)
fn ic_metin(s: &str, t: &[(Bt, usize, usize)], bas: usize, son: usize) -> String {
    if bas >= son {
        return String::new();
    }
    s.get(t[bas].1..t[son - 1].2).unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Virgülle ayrılmış ad listesi: `(a, b DESC, "c")`
fn ad_listesi(t: &[(Bt, usize, usize)], bas: usize, son: usize) -> Vec<String> {
    let mut v = Vec::new();
    let mut derinlik = 0;
    let mut ilk = true;
    for (b, _, _) in &t[bas..son] {
        match b {
            Bt::Isaret('(') => derinlik += 1,
            Bt::Isaret(')') => derinlik -= 1,
            Bt::Isaret(',') if derinlik == 0 => ilk = true,
            Bt::Ad(a, _) if ilk && derinlik == 0 => {
                v.push(a.clone());
                ilk = false;
            }
            _ => ilk = false,
        }
    }
    v
}

/// Nesne adı (`sema.ad` ise son parça) ve sonraki konum
fn nesne_adi(t: &[(Bt, usize, usize)], mut i: usize) -> Option<(String, usize)> {
    let mut a = ad(t.get(i))?;
    i += 1;
    while t.get(i).map(|x| &x.0) == Some(&Bt::Isaret('.')) {
        a = ad(t.get(i + 1))?;
        i += 2;
    }
    Some((a, i))
}

/// `IF NOT EXISTS` atlanır
fn varsa_atla(t: &[(Bt, usize, usize)], i: usize) -> usize {
    if kelime(t.get(i), "IF") && kelime(t.get(i + 1), "NOT") && kelime(t.get(i + 2), "EXISTS") {
        i + 3
    } else {
        i
    }
}

fn kisalt(s: &str) -> String {
    if s.chars().count() > EN_UZUN_METIN {
        format!("{}…", s.chars().take(EN_UZUN_METIN).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Sütun tanımını bitiren kısıt anahtar kelimeleri
const KISITLAR: &[&str] = &["CONSTRAINT", "PRIMARY", "NOT", "NULL", "UNIQUE", "CHECK", "DEFAULT", "COLLATE", "REFERENCES", "GENERATED", "AS"];

/// `REFERENCES tablo (sütunlar) [ON DELETE x] ...`: (tablo, sütunlar, silinince, sonraki konum)
fn referans(t: &[(Bt, usize, usize)], i: usize) -> Option<(Value, usize)> {
    let (tablo, mut j) = nesne_adi(t, i)?;
    let mut sutunlar = Vec::new();
    if let Some((b, s, sonra)) = parantez(t, j) {
        sutunlar = ad_listesi(t, b, s);
        j = sonra;
    }
    let mut silinince = None;
    let mut guncellenince = None;
    while kelime(t.get(j), "ON") {
        let olay = ad(t.get(j + 1)).unwrap_or_default().to_uppercase();
        let mut eylem = vec![ad(t.get(j + 2)).unwrap_or_default().to_uppercase()];
        j += 3;
        if matches!(eylem[0].as_str(), "SET" | "NO") {
            eylem.push(ad(t.get(j)).unwrap_or_default().to_uppercase());
            j += 1;
        }
        let e = eylem.join(" ");
        if olay == "DELETE" {
            silinince = Some(e);
        } else {
            guncellenince = Some(e);
        }
    }
    Some((json!({ "tablo": tablo, "sutunlar": sutunlar, "silinince": silinince, "guncellenince": guncellenince }), j))
}

/// CREATE TABLE metni → tablo tanımı (sütunlar, CHECK, FK, birincil anahtar). Çözümlenemeyen kısım `None` değil,
/// `cozulemeyen` listesine yazılır.
pub(crate) fn tablo(sql: &str) -> Value {
    let t = belirtecler(sql);
    let mut cozulemeyen = Vec::new();
    let sanal = kelime(t.get(1), "VIRTUAL");
    let Some(i) = t.iter().position(|x| kelime(Some(x), "TABLE")) else {
        return json!({ "cozulemeyen": ["CREATE TABLE değil"] });
    };
    let i = varsa_atla(&t, i + 1);
    let Some((tablo_adi, i)) = nesne_adi(&t, i) else {
        return json!({ "cozulemeyen": ["tablo adı okunamadı"] });
    };
    if sanal {
        return json!({ "ad": tablo_adi, "sanal": ad(t.get(i + 1)).filter(|_| kelime(t.get(i), "USING")) });
    }
    let Some((bas, son, sonra)) = parantez(&t, i) else {
        return json!({ "ad": tablo_adi, "cozulemeyen": ["sütun listesi yok (CREATE TABLE … AS SELECT)"] });
    };
    let mut sutunlar = Vec::new();
    let mut kontroller = Vec::new();
    let mut yabanci = Vec::new();
    let mut birincil: Vec<String> = Vec::new();
    let mut benzersiz = Vec::new();
    // Üst düzey virgüllerle parçalar
    let mut parcalar = Vec::new();
    let (mut p, mut derinlik) = (bas, 0);
    for (k, x) in t.iter().enumerate().take(son).skip(bas) {
        match x.0 {
            Bt::Isaret('(') => derinlik += 1,
            Bt::Isaret(')') => derinlik -= 1,
            Bt::Isaret(',') if derinlik == 0 => {
                parcalar.push((p, k));
                p = k + 1;
            }
            _ => {}
        }
    }
    parcalar.push((p, son));
    for (b, s) in parcalar {
        if b >= s {
            continue;
        }
        let mut j = b;
        if kelime(t.get(j), "CONSTRAINT") {
            j += 2;
        }
        let ac = t.get(j + 1).map(|x| &x.0) == Some(&Bt::Isaret('('));
        let tablo_kisiti =
            ((kelime(t.get(j), "PRIMARY") || kelime(t.get(j), "FOREIGN")) && kelime(t.get(j + 1), "KEY")) || ((kelime(t.get(j), "UNIQUE") || kelime(t.get(j), "CHECK")) && ac);
        if tablo_kisiti {
            if kelime(t.get(j), "CHECK") {
                match parantez(&t, j + 1) {
                    Some((x, y, _)) => kontroller.push(kisalt(&ic_metin(sql, &t, x, y))),
                    None => cozulemeyen.push(kisalt(&ic_metin(sql, &t, b, s))),
                }
            } else if kelime(t.get(j), "FOREIGN") {
                let r = parantez(&t, j + 2).and_then(|(x, y, sonra)| {
                    let kaynak = ad_listesi(&t, x, y);
                    kelime(t.get(sonra), "REFERENCES").then_some(())?;
                    let (mut r, _) = referans(&t, sonra + 1)?;
                    r["kaynak_sutunlar"] = json!(kaynak);
                    Some(r)
                });
                match r {
                    Some(r) => yabanci.push(r),
                    None => cozulemeyen.push(kisalt(&ic_metin(sql, &t, b, s))),
                }
            } else {
                let liste = parantez(&t, j + if kelime(t.get(j), "PRIMARY") { 2 } else { 1 }).map(|(x, y, _)| ad_listesi(&t, x, y)).unwrap_or_default();
                if kelime(t.get(j), "PRIMARY") {
                    birincil = liste;
                } else {
                    benzersiz.push(json!(liste));
                }
            }
            continue;
        }
        // Sütun: ad, tür (kısıt anahtar kelimesine kadar), kısıtlar
        let Some(sutun) = ad(t.get(j)) else {
            cozulemeyen.push(kisalt(&ic_metin(sql, &t, b, s)));
            continue;
        };
        j += 1;
        let tur_bas = j;
        while j < s && !KISITLAR.iter().any(|k| kelime(t.get(j), k)) {
            if let Some((_, _, sonra)) = parantez(&t, j) {
                j = sonra.min(s);
            } else {
                j += 1;
            }
        }
        let tur = ic_metin(sql, &t, tur_bas, j);
        let mut sut = json!({ "ad": sutun, "tur": tur, "bos_olamaz": false, "birincil": false, "benzersiz": false, "varsayilan": false, "kontroller": [] });
        while j < s {
            if kelime(t.get(j), "CONSTRAINT") {
                j += 2;
            } else if kelime(t.get(j), "PRIMARY") {
                sut["birincil"] = json!(true);
                birincil = vec![sutun.clone()];
                j += 2;
            } else if kelime(t.get(j), "NOT") && kelime(t.get(j + 1), "NULL") {
                sut["bos_olamaz"] = json!(true);
                j += 2;
            } else if kelime(t.get(j), "UNIQUE") {
                sut["benzersiz"] = json!(true);
                j += 1;
            } else if kelime(t.get(j), "CHECK") {
                match parantez(&t, j + 1) {
                    Some((x, y, sonra)) => {
                        sut["kontroller"].as_array_mut().unwrap().push(json!(kisalt(&ic_metin(sql, &t, x, y))));
                        kontroller.push(kisalt(&ic_metin(sql, &t, x, y)));
                        j = sonra;
                    }
                    None => j = s,
                }
            } else if kelime(t.get(j), "DEFAULT") {
                sut["varsayilan"] = json!(true);
                j = parantez(&t, j + 1).map(|(_, _, sonra)| sonra).unwrap_or(j + 2 + usize::from(matches!(t.get(j + 1).map(|x| &x.0), Some(Bt::Isaret('-' | '+')))));
            } else if kelime(t.get(j), "REFERENCES") {
                match referans(&t, j + 1) {
                    Some((mut r, sonra)) => {
                        r["kaynak_sutunlar"] = json!([sutun]);
                        sut["referans"] = r.clone();
                        yabanci.push(r);
                        j = sonra;
                    }
                    None => j = s,
                }
            } else if kelime(t.get(j), "AS") || kelime(t.get(j), "GENERATED") {
                sut["uretilmis"] = json!(true);
                j += 1;
            } else {
                j += 1;
            }
        }
        sutunlar.push(sut);
    }
    let rowidsiz = t.iter().skip(sonra).any(|x| kelime(Some(x), "WITHOUT"));
    json!({
        "ad": tablo_adi,
        "sutunlar": sutunlar,
        "birincil_anahtar": birincil,
        "benzersiz": benzersiz,
        "kontroller": kontroller,
        "yabanci_anahtarlar": yabanci,
        "rowidsiz": rowidsiz,
        "cozulemeyen": cozulemeyen,
    })
}

/// CREATE INDEX metni → (benzersiz mi, sütunlar, kısmi mi)
pub(crate) fn indeks(sql: &str) -> Value {
    let t = belirtecler(sql);
    let benzersiz = kelime(t.get(1), "UNIQUE");
    let sutunlar = t
        .iter()
        .position(|x| kelime(Some(x), "ON"))
        .and_then(|i| nesne_adi(&t, i + 1))
        .and_then(|(_, j)| parantez(&t, j))
        .map(|(x, y, sonra)| (ad_listesi(&t, x, y), kelime(t.get(sonra), "WHERE")));
    match sutunlar {
        Some((s, kismi)) => json!({ "benzersiz": benzersiz, "sutunlar": s, "kismi": kismi }),
        None => json!({ "benzersiz": benzersiz, "cozulemeyen": true }),
    }
}

/// CREATE TRIGGER metni → (zaman, olay, sütunlar); gövde çalıştırılmaz, metin olarak saklanır
pub(crate) fn tetikleyici(sql: &str) -> Value {
    let t = belirtecler(sql);
    let mut zaman = "BEFORE".to_string();
    let mut olay = None;
    let mut sutunlar = Vec::new();
    for (i, _) in t.iter().enumerate().take(64) {
        if kelime(t.get(i), "BEFORE") || kelime(t.get(i), "AFTER") {
            zaman = ad(t.get(i)).unwrap_or_default().to_uppercase();
        } else if kelime(t.get(i), "INSTEAD") {
            zaman = "INSTEAD OF".into();
        } else if olay.is_none() && ["INSERT", "UPDATE", "DELETE"].iter().any(|k| kelime(t.get(i), k)) {
            olay = ad(t.get(i)).map(|a| a.to_uppercase());
            if kelime(t.get(i + 1), "OF") {
                let mut j = i + 2;
                while let Some(a) = ad(t.get(j)).filter(|_| !kelime(t.get(j), "ON")) {
                    sutunlar.push(a);
                    j += 1;
                    if t.get(j).map(|x| &x.0) == Some(&Bt::Isaret(',')) {
                        j += 1;
                    }
                }
            }
        } else if kelime(t.get(i), "ON") && olay.is_some() {
            break;
        }
    }
    json!({ "zaman": zaman, "olay": olay, "sutunlar": sutunlar, "metin": kisalt(sql), "calistirilmadi": true })
}

/// Şemanın biçim projeksiyonu: tablo başına bir öğe (indeks ve tetikleyiciler tablosunun altında), görünümler ayrı
pub(crate) fn tablolar(s: &Sema) -> (Vec<Value>, Vec<Value>) {
    let mut tablolar: Vec<Value> = Vec::new();
    let mut gorunumler = Vec::new();
    for (tur, ad, _, sql) in &s.satirlar {
        if tur == "table" && !ad.starts_with("sqlite_") {
            let mut t = sql.as_deref().map(tablo).unwrap_or_else(|| json!({ "cozulemeyen": ["CREATE metni yok"] }));
            t["ad"] = json!(ad);
            t["indeksler"] = json!([]);
            t["tetikleyiciler"] = json!([]);
            tablolar.push(t);
        } else if tur == "view" {
            gorunumler.push(json!({ "ad": ad, "metin": sql.as_deref().map(kisalt), "calistirilmadi": true }));
        }
    }
    for (tur, ad, tablo_adi, sql) in &s.satirlar {
        let Some(t) = tablolar.iter_mut().find(|t| t["ad"].as_str().is_some_and(|a| a.eq_ignore_ascii_case(tablo_adi))) else { continue };
        match tur.as_str() {
            "index" => {
                let mut i = sql.as_deref().map(indeks).unwrap_or_else(|| json!({ "otomatik": true }));
                i["ad"] = json!(ad);
                t["indeksler"].as_array_mut().unwrap().push(i);
            }
            "trigger" => {
                let mut i = sql.as_deref().map(tetikleyici).unwrap_or_else(|| json!({}));
                i["ad"] = json!(ad);
                t["tetikleyiciler"].as_array_mut().unwrap().push(i);
            }
            _ => {}
        }
    }
    (tablolar, gorunumler)
}
