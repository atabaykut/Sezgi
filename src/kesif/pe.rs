//! PE (Windows `.exe` / `.dll`) okuyucu: kaynak kodu olmayan bir uygulamanın kimliği. Salt okunur, hiçbir kod
//! yüklenmez ya da çalıştırılmaz; her okuma sınır denetimlidir (bozuk ya da kasıtlı kötü dosya panik değil `Err`/sorun
//! verir).
//! Çıkanlar: makine, alt sistem, PE32/PE32+, DLL mi, .NET CLI başlığı (varsa meta veri sürümü), sürüm kaynağı
//! (FileVersion, ProductName, CompanyName …) ve import edilen DLL adları. Kanıt türü BINARY_METADATA (tavan observed).
//! Hedef çatı (`.NETCoreApp,Version=v8.0`) dosyadaki dizeden okunur: yorumdur, BINARY_HEURISTIC (tavan inferred).
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Okunacak en büyük dosya (daha büyüğü okunmaz, sorun olarak bildirilir)
pub(crate) const EN_BUYUK: u64 = 512 << 20;
/// Sürüm kaynağından alınan alanlar (başkaları okunmaz)
const SURUM_ALANLARI: &[&str] = &["CompanyName", "FileDescription", "FileVersion", "InternalName", "LegalCopyright", "OriginalFilename", "ProductName", "ProductVersion"];
/// Bir dosyada en fazla bu kadar import edilen DLL ve bölüm okunur
const EN_FAZLA_IMPORT: usize = 1024;
const EN_FAZLA_BOLUM: usize = 96;

/// PE'nin kimlik özeti
#[derive(Debug, Default)]
pub(crate) struct Pe {
    pub makine: String,
    pub alt_sistem: String,
    pub pe32_arti: bool,
    pub dll: bool,
    /// .NET CLI başlığı var mı (yönetilen kod)
    pub dotnet: bool,
    /// CLI meta veri kökündeki sürüm dizgisi (ör. v4.0.30319)
    pub clr_surumu: Option<String>,
    /// VS_FIXEDFILEINFO dosya sürümü
    pub sabit_surum: Option<String>,
    pub surum_kaynagi: BTreeMap<String, String>,
    pub importlar: Vec<String>,
    pub bolumler: Vec<String>,
    /// Dizeden okunan hedef çatı (yorum)
    pub hedef_cati: Option<String>,
    /// Okunamayan ya da tutarsız kısımlar (kanıta "bilinmiyor" olarak girer)
    pub sorunlar: Vec<String>,
}

impl Pe {
    /// Kanıtın biçim projeksiyonu (BINARY_METADATA)
    pub fn ozet(&self) -> Value {
        json!({
            "bicim": "pe",
            "makine": self.makine,
            "alt_sistem": self.alt_sistem,
            "pe32_arti": self.pe32_arti,
            "dll": self.dll,
            "dotnet": self.dotnet,
            "clr_surumu": self.clr_surumu,
            "dosya_surumu": self.sabit_surum,
            "surum_kaynagi": self.surum_kaynagi,
            "importlar": self.importlar,
            "bolumler": self.bolumler,
            "sorunlar": self.sorunlar,
        })
    }
}

/// Sınır denetimli okuyucu: her okuma `Option` döner, taşma `None` olur
struct Okur<'a>(&'a [u8]);

impl<'a> Okur<'a> {
    fn dilim(&self, o: usize, n: usize) -> Option<&'a [u8]> {
        self.0.get(o..o.checked_add(n)?)
    }
    fn u16(&self, o: usize) -> Option<u16> {
        self.dilim(o, 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&self, o: usize) -> Option<u32> {
        self.dilim(o, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// Sıfırla biten ASCII ad (en fazla `en_fazla` bayt; yazdırılamaz karakter varsa `None`)
    fn c_dizgi(&self, o: usize, en_fazla: usize) -> Option<String> {
        let b = self.0.get(o..)?;
        let son = b.iter().take(en_fazla).position(|&c| c == 0)?;
        let s = &b[..son];
        (!s.is_empty() && s.iter().all(|c| c.is_ascii_graphic() || *c == b' ')).then(|| String::from_utf8_lossy(s).into_owned())
    }
}

/// Bölüm tablosu satırı
struct Bolum {
    va: u32,
    sanal: u32,
    ham_boyut: u32,
    ham_konum: u32,
}

/// RVA -> dosya konumu (bölümlerden; hiçbirine düşmüyorsa `None`)
fn konum(bolumler: &[Bolum], rva: u32, uzunluk: usize) -> Option<usize> {
    for b in bolumler {
        let kapsam = b.sanal.max(b.ham_boyut);
        if rva >= b.va && rva - b.va < kapsam {
            let o = (b.ham_konum as usize).checked_add((rva - b.va) as usize)?;
            return (o < uzunluk && rva - b.va < b.ham_boyut).then_some(o);
        }
    }
    None
}

fn makine_adi(m: u16) -> String {
    match m {
        0x014c => "x86".into(),
        0x8664 => "x64".into(),
        0xaa64 => "arm64".into(),
        0x01c4 => "arm".into(),
        0x0200 => "ia64".into(),
        0 => "belirsiz".into(),
        x => format!("0x{x:04x}"),
    }
}

fn alt_sistem_adi(s: u16) -> String {
    match s {
        1 => "native".into(),
        2 => "windows_gui".into(),
        3 => "windows_cui".into(),
        9 => "windows_ce_gui".into(),
        10 => "efi_uygulamasi".into(),
        16 => "windows_onyukleme".into(),
        x => format!("{x}"),
    }
}

/// PE dosyasını okur. MZ/PE imzası yoksa `Err`; imzadan sonraki her bozukluk `sorunlar`a yazılır, okunabilen kısım döner.
pub(crate) fn oku(veri: &[u8]) -> Result<Pe, String> {
    let r = Okur(veri);
    if r.dilim(0, 2) != Some(b"MZ") {
        return Err("MZ imzası yok: PE değil".into());
    }
    let pe = r.u32(0x3c).ok_or("DOS başlığı kısa")? as usize;
    if r.dilim(pe, 4) != Some(b"PE\0\0") {
        return Err("PE imzası yok (yalnızca DOS başlığı)".into());
    }
    let coff = pe + 4;
    let mut p = Pe { makine: makine_adi(r.u16(coff).ok_or("COFF başlığı kısa")?), ..Pe::default() };
    let bolum_sayisi = r.u16(coff + 2).unwrap_or(0) as usize;
    let secenek_boyu = r.u16(coff + 16).unwrap_or(0) as usize;
    p.dll = r.u16(coff + 18).is_some_and(|c| c & 0x2000 != 0);
    let sec = coff + 20;
    let (dizin_sayisi_konumu, dizinler) = match r.u16(sec) {
        Some(0x10b) => (sec + 92, sec + 96),
        Some(0x20b) => {
            p.pe32_arti = true;
            (sec + 108, sec + 112)
        }
        _ => {
            p.sorunlar.push("isteğe bağlı başlık tanınmadı (PE32/PE32+ değil)".into());
            return Ok(p);
        }
    };
    p.alt_sistem = r.u16(sec + 68).map(alt_sistem_adi).unwrap_or_else(|| "bilinmiyor".into());
    let dizin_sayisi = r.u32(dizin_sayisi_konumu).unwrap_or(0).min(16) as usize;
    let dizin = |i: usize| -> Option<(u32, u32)> {
        if i >= dizin_sayisi || dizinler + i * 8 + 8 > sec + secenek_boyu {
            return None;
        }
        Some((r.u32(dizinler + i * 8)?, r.u32(dizinler + i * 8 + 4)?)).filter(|(rva, boy)| *rva != 0 && *boy != 0)
    };
    // Bölüm tablosu
    let tablo = sec + secenek_boyu;
    let mut bolumler = Vec::new();
    for i in 0..bolum_sayisi.min(EN_FAZLA_BOLUM) {
        let o = tablo + i * 40;
        let (Some(ad), Some(sanal), Some(va), Some(ham_boyut), Some(ham_konum)) = (r.dilim(o, 8), r.u32(o + 8), r.u32(o + 12), r.u32(o + 16), r.u32(o + 20)) else {
            p.sorunlar.push("bölüm tablosu dosyanın sonunu aşıyor".into());
            break;
        };
        let ad: String = ad.iter().take_while(|&&c| c != 0).map(|&c| if c.is_ascii_graphic() { c as char } else { '?' }).collect();
        p.bolumler.push(ad);
        bolumler.push(Bolum { va, sanal, ham_boyut, ham_konum });
    }
    if bolum_sayisi > EN_FAZLA_BOLUM {
        p.sorunlar.push(format!("{bolum_sayisi} bölüm: ilk {EN_FAZLA_BOLUM} okundu"));
    }
    let k = |rva: u32| konum(&bolumler, rva, veri.len());
    // Import edilen DLL'ler (dizin 1)
    if let Some((rva, _)) = dizin(1) {
        match k(rva) {
            Some(mut o) => {
                for _ in 0..EN_FAZLA_IMPORT {
                    let Some(ad_rva) = r.u32(o + 12) else {
                        p.sorunlar.push("import tablosu dosyanın sonunu aşıyor".into());
                        break;
                    };
                    if ad_rva == 0 {
                        break;
                    }
                    match k(ad_rva).and_then(|a| r.c_dizgi(a, 256)) {
                        Some(ad) if !p.importlar.contains(&ad) => p.importlar.push(ad),
                        Some(_) => {}
                        None => p.sorunlar.push("okunamayan import adı".into()),
                    }
                    o += 20;
                }
            }
            None => p.sorunlar.push("import tablosu hiçbir bölüme düşmüyor".into()),
        }
    }
    // .NET CLI başlığı (dizin 14) ve meta veri kökü
    if let Some((rva, _)) = dizin(14) {
        p.dotnet = true;
        let surum = k(rva).and_then(|o| r.u32(o + 8)).and_then(k).and_then(|m| {
            (r.u32(m)? == 0x424a_5342).then_some(())?;
            let n = (r.u32(m + 12)? as usize).min(256);
            r.c_dizgi(m + 16, n)
        });
        match surum {
            Some(s) => p.clr_surumu = Some(s),
            None => p.sorunlar.push("CLI meta veri kökü okunamadı".into()),
        }
    }
    // Sürüm kaynağı (dizin 2, RT_VERSION)
    if let Some((rva, _)) = dizin(2) {
        if let Some(v) = k(rva).and_then(|taban| surum_kaynagi_bul(&r, taban, &k)) {
            surum_bilgisi(v, &mut p);
        }
    }
    p.hedef_cati = hedef_cati(veri);
    Ok(p)
}

/// Kaynak ağacında RT_VERSION (16) → ilk ad → ilk dil → veri girdisi; sürüm bloğunun baytları
fn surum_kaynagi_bul<'a>(r: &Okur<'a>, taban: usize, k: &dyn Fn(u32) -> Option<usize>) -> Option<&'a [u8]> {
    // Dizin girdileri: (ad/kimlik, konum); yüksek bit alt dizin demektir
    let girdiler = |o: usize| -> Option<Vec<(u32, u32)>> {
        let n = (r.u16(o + 12)? as usize + r.u16(o + 14)? as usize).min(4096);
        (0..n).map(|i| Some((r.u32(o + 16 + i * 8)?, r.u32(o + 20 + i * 8)?))).collect()
    };
    let (_, tur) = *girdiler(taban)?.iter().find(|(ad, _)| *ad == 16)?;
    let alt = |g: u32| (g & 0x8000_0000 != 0).then(|| taban.checked_add((g & 0x7fff_ffff) as usize)).flatten();
    let (_, ad) = *girdiler(alt(tur)?)?.first()?;
    let (_, dil) = *girdiler(alt(ad)?)?.first()?;
    if dil & 0x8000_0000 != 0 {
        return None;
    }
    let veri = taban.checked_add(dil as usize)?;
    let (v_rva, v_boy) = (r.u32(veri)?, r.u32(veri + 4)?);
    r.dilim(k(v_rva)?, (v_boy as usize).min(64 << 10))
}

/// VS_VERSIONINFO bloğu: (anahtar, değer başı, değer sonu (blok sonu), çocukların başı, blok sonu)
fn blok(v: &[u8], bas: usize, ust_son: usize) -> Option<(String, usize, usize, usize)> {
    let r = Okur(v);
    let uzunluk = r.u16(bas)? as usize;
    let deger_uzunlugu = r.u16(bas + 2)? as usize;
    let tur = r.u16(bas + 4)?;
    let son = bas.checked_add(uzunluk)?;
    if uzunluk < 6 || son > ust_son || son > v.len() {
        return None;
    }
    let mut o = bas + 6;
    let mut anahtar = Vec::new();
    while o + 2 <= son {
        let c = r.u16(o)?;
        o += 2;
        if c == 0 {
            break;
        }
        anahtar.push(c);
    }
    let hizali = |x: usize| (x + 3) & !3;
    let deger = hizali(o).min(son);
    let deger_bayt = if tur == 1 { deger_uzunlugu * 2 } else { deger_uzunlugu };
    let cocuk = hizali(deger.saturating_add(deger_bayt)).min(son);
    Some((String::from_utf16_lossy(&anahtar), deger, cocuk, son))
}

/// Sürüm bloğundan sabit dosya sürümü ve dize tablosu
fn surum_bilgisi(v: &[u8], p: &mut Pe) {
    let r = Okur(v);
    let Some((anahtar, deger, cocuk, son)) = blok(v, 0, v.len()) else {
        p.sorunlar.push("sürüm kaynağı okunamadı".into());
        return;
    };
    if anahtar != "VS_VERSION_INFO" {
        p.sorunlar.push("sürüm kaynağının anahtarı beklenmedik".into());
        return;
    }
    if r.u32(deger) == Some(0xfeef_04bd) {
        if let (Some(ms), Some(ls)) = (r.u32(deger + 8), r.u32(deger + 12)) {
            p.sabit_surum = Some(format!("{}.{}.{}.{}", ms >> 16, ms & 0xffff, ls >> 16, ls & 0xffff));
        }
    }
    // Çocuklar: StringFileInfo → dize tabloları → dizeler (en fazla 3 düzey; her döngü ilerler ya da durur)
    let mut c = cocuk;
    while let Some((ad, _, alt, bitis)) = blok(v, c, son) {
        if ad == "StringFileInfo" {
            let mut t = alt;
            while let Some((_, _, dizeler, tablo_sonu)) = blok(v, t, bitis) {
                let mut d = dizeler;
                while let Some((alan, deger, _, dize_sonu)) = blok(v, d, tablo_sonu) {
                    if SURUM_ALANLARI.contains(&alan.as_str()) {
                        let birimler: Vec<u16> = (deger..dize_sonu.saturating_sub(1)).step_by(2).map_while(|i| r.u16(i)).take_while(|&c| c != 0).take(512).collect();
                        let metin = crate::guvenli::temiz(String::from_utf16_lossy(&birimler).trim());
                        if !metin.is_empty() {
                            p.surum_kaynagi.insert(alan, metin);
                        }
                    }
                    d = (dize_sonu + 3) & !3;
                }
                t = (tablo_sonu + 3) & !3;
            }
        }
        c = (bitis + 3) & !3;
    }
}

/// `TargetFrameworkAttribute` dizesi (ör. `.NETCoreApp,Version=v8.0`): ikilideki dizeden okunur, yorumdur
fn hedef_cati(veri: &[u8]) -> Option<String> {
    for onek in [&b".NETCoreApp,Version=v"[..], b".NETFramework,Version=v", b".NETStandard,Version=v"] {
        if let Some(i) = veri.windows(onek.len()).position(|w| w == onek) {
            let surum: String = veri[i + onek.len()..].iter().take(16).take_while(|c| c.is_ascii_digit() || **c == b'.').map(|&c| c as char).collect();
            if !surum.is_empty() {
                return Some(format!("{}{surum}", String::from_utf8_lossy(onek)));
            }
        }
    }
    None
}
