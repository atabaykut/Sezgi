//! Çıktı dili: Türkçe (varsayılan) ya da İngilizce. Süreç boyunca tek seçim yapılır.
//!
//! Öncelik sırası:
//! 1. `--lang en|tr` bayrağı (bütün komutlarda, her konumda; eski `--dil` de kabul edilir). `main` bayrağı komut
//!    satırından çıkarır, komutlar onu görmez.
//! 2. `SEZGI_LANG` ortam değişkeni (`en`, `tr`, `en_US` gibi).
//! 3. Sistem dili: Windows'ta kullanıcının arayüz dili (`GetUserDefaultUILanguage`), belirlenemezse ve diğer
//!    sistemlerde `LC_ALL`, `LC_MESSAGES`, `LANG` (ilk dolu olan). Türkçeyse Türkçe, açıkça başka bir dilse İngilizce.
//! 4. Belirlenemezse (`C`, `POSIX`, boş) Türkçe.
//!
//! Çeviri, metnin yanında yazılır ve derleme zamanında denetlenir:
//! - `t!("tr", "en")`: iki değerden seçilen (çoğunlukla `&'static str`)
//! - `tf!("tr {x}", "en {x}", ...)`: `format!` ile aynı (yakalanan adlar ve ek argümanlar)
//! - `tp!(...)` / `te!(...)`: `println!` / `eprintln!` ile aynı
//! - Üç makroda da `;` sonrası yalnızca İngilizce metne giden adlı argümanlar: `tf!("{n} tablo", "{n} {t}"; t =
//!   cogul(n, "table", "tables"))`
//! - İngilizcede sayıya uyan ad ya da fiil: [`cogul`] ("1 table", "2 tables"; "is"/"are"), [`sayili`] (iki dilde sayı ve ad)
//!
//! Makine okunur çıktı (JSON alanları, dosya biçimleri, komut ve bayrak adları) dil seçiminden etkilenmez.
use std::sync::OnceLock;

/// İki dilden seçilen değer: `t!("Türkçe", "English")`
macro_rules! t {
    ($tr:expr, $en:expr $(,)?) => {
        if $crate::dil::en() {
            $en
        } else {
            $tr
        }
    };
}

/// Seçilen dilde `format!`: `tf!("{n} işlem", "{n} operations")`, ek argümanlar iki metne de verilir
macro_rules! tf {
    ($tr:literal, $en:literal $(, $a:expr)* $(,)? $(; $($ad:ident = $e:expr),+ $(,)?)?) => {
        if $crate::dil::en() {
            format!($en $(, $a)* $($(, $ad = $e)+)?)
        } else {
            format!($tr $(, $a)*)
        }
    };
}

/// Seçilen dilde `println!`
macro_rules! tp {
    ($tr:literal, $en:literal $(, $a:expr)* $(,)? $(; $($ad:ident = $e:expr),+ $(,)?)?) => {
        if $crate::dil::en() {
            println!($en $(, $a)* $($(, $ad = $e)+)?)
        } else {
            println!($tr $(, $a)*)
        }
    };
}

/// Seçilen dilde `eprintln!`
macro_rules! te {
    ($tr:literal, $en:literal $(, $a:expr)* $(,)? $(; $($ad:ident = $e:expr),+ $(,)?)?) => {
        if $crate::dil::en() {
            eprintln!($en $(, $a)* $($(, $ad = $e)+)?)
        } else {
            eprintln!($tr $(, $a)*)
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dil {
    Tr,
    En,
}

static SECIM: OnceLock<Dil> = OnceLock::new();

thread_local! {
    /// İstek başına dil (Ayna'nın her isteği kendi dilinde cevaplanır): `ile` süresince bu iş parçacığında geçerli
    static YEREL: std::cell::Cell<Option<Dil>> = const { std::cell::Cell::new(None) };
}

/// Seçilen dil: bu iş parçacığında `ile` ile seçilmişse o, yoksa sürecin dili
pub fn dil() -> Dil {
    YEREL.with(|y| y.get()).unwrap_or_else(|| *SECIM.get_or_init(ortamdan))
}

/// `f`'yi bu iş parçacığında `d` diliyle çalıştırır; önceki seçim sonra (panikte de) geri gelir
pub fn ile<T>(d: Dil, f: impl FnOnce() -> T) -> T {
    struct Geri(Option<Dil>);
    impl Drop for Geri {
        fn drop(&mut self) {
            YEREL.with(|y| y.set(self.0));
        }
    }
    let _geri = Geri(YEREL.with(|y| y.replace(Some(d))));
    f()
}

/// Dil etiketi (`en`, `tr`, `en_US`, `english`); tanınmazsa None
pub fn etiketten(s: &str) -> Option<Dil> {
    etiket(s)
}

/// Bayrak yokken: `SEZGI_LANG`, sistem dili, Türkçe
fn ortamdan() -> Dil {
    belirle(None, std::env::var("SEZGI_LANG").ok().as_deref(), sistem_dili())
}

/// Çıktı İngilizce mi
pub fn en() -> bool {
    dil() == Dil::En
}

/// İki metinden seçilen dildeki
pub fn sec<'a>(tr: &'a str, en: &'a str) -> &'a str {
    if self::en() {
        en
    } else {
        tr
    }
}

/// Sayılabilen bir değer (adet): İngilizcede ad 1 için tekil, başka her sayı için çoğul
pub trait Adet {
    fn bir_mi(&self) -> bool;
}

macro_rules! adet_tamsayi {
    ($($t:ty),*) => {
        $(impl Adet for $t {
            fn bir_mi(&self) -> bool {
                *self == 1
            }
        })*
    };
}
adet_tamsayi!(usize, u64, u32, i64, i32);

/// Kural sınırları (uzunluk, öğe sayısı) kesirli sayı olarak tutulur: tam 1 tekildir
impl Adet for f64 {
    fn bir_mi(&self) -> bool {
        (*self - 1.0).abs() < f64::EPSILON
    }
}

impl Adet for serde_json::Value {
    fn bir_mi(&self) -> bool {
        self.as_u64() == Some(1)
    }
}

impl<T: Adet + ?Sized> Adet for &T {
    fn bir_mi(&self) -> bool {
        (**self).bir_mi()
    }
}

/// Sayıya uyan İngilizce biçim: `cogul(n, "table", "tables")` 1 için "table", başka sayılar için "tables". Fiil uyumu
/// için de: `cogul(n, "is", "are")`. Türkçede ad sayıdan sonra çoğul eki almadığı için yalnızca İngilizce metinde kullanılır
pub fn cogul<'a>(n: impl Adet, tekil: &'a str, cok: &'a str) -> &'a str {
    if n.bir_mi() {
        tekil
    } else {
        cok
    }
}

/// Seçilen dilde sayı ve ad: Türkçede "3 tablo" (ad değişmez), İngilizcede "1 table" / "3 tables"
pub fn sayili(n: impl Adet + std::fmt::Display, tr: &str, tekil: &str, cok: &str) -> String {
    if en() {
        format!("{n} {}", cogul(&n, tekil, cok))
    } else {
        format!("{n} {tr}")
    }
}

/// Komut satırındaki `--lang <dil>` / `--lang=<dil>` (ve `--dil`) bayrağını okur, satırdan çıkarır ve süreç dilini
/// kurar. Geçersiz değer kullanım hatasıdır (çıkış kodu 2). Komut çalışmadan, ilk çıktıdan önce çağrılır
pub fn kur(args: &mut Vec<String>) {
    let mut bayrak: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--lang" || a == "--dil" {
            let Some(d) = args.get(i + 1).filter(|d| !d.starts_with("--")).cloned() else {
                te!("✗ --lang bir değer ister: en ya da tr", "✗ --lang needs a value: en or tr");
                std::process::exit(2);
            };
            bayrak = Some(d);
            args.drain(i..i + 2);
        } else if let Some(d) = a.strip_prefix("--lang=").or_else(|| a.strip_prefix("--dil=")) {
            bayrak = Some(d.to_string());
            args.remove(i);
        } else {
            i += 1;
        }
    }
    let secilen = match bayrak.as_deref() {
        Some(b) => match etiket(b) {
            Some(d) => d,
            None => {
                te!("✗ --lang yalnızca en ya da tr alır: {b}", "✗ --lang accepts only en or tr: {b}");
                std::process::exit(2);
            }
        },
        None => ortamdan(),
    };
    let _ = SECIM.set(secilen);
}

/// Öncelik kuralı: bayrak > `SEZGI_LANG` > sistem dili > Türkçe. Tanınmayan bayrak ya da `SEZGI_LANG` değeri bir
/// sonrakine bırakır
pub fn belirle(bayrak: Option<&str>, ortam: Option<&str>, sistem: Option<Dil>) -> Dil {
    bayrak.and_then(etiket).or_else(|| ortam.and_then(etiket)).or(sistem).unwrap_or(Dil::Tr)
}

/// Açık dil etiketi: `tr`, `tr_TR`, `tr-TR`, `turkce` → Türkçe; `en`, `en_US`, `english` → İngilizce; başka bir şey
/// tanınmaz
fn etiket(s: &str) -> Option<Dil> {
    let k = s.trim().to_ascii_lowercase();
    let on = k.split(['_', '-', '.', '@']).next().unwrap_or("");
    match on {
        "tr" | "turkce" | "turkish" => Some(Dil::Tr),
        "en" | "ingilizce" | "english" => Some(Dil::En),
        _ => None,
    }
}

/// Yerel ayar değerinden dil (`LANG=de_DE.UTF-8` gibi): Türkçe → Türkçe; `C`, `POSIX` ya da boş → belirlenemedi;
/// başka bir dil → İngilizce
pub fn yerel_ayardan(deger: &str) -> Option<Dil> {
    let k = deger.trim();
    let on = k.split(['_', '-', '.', '@']).next().unwrap_or("").to_ascii_lowercase();
    match on.as_str() {
        "" | "c" | "posix" => None,
        "tr" => Some(Dil::Tr),
        _ if on.len() >= 2 && on.chars().all(|c| c.is_ascii_alphabetic()) => Some(Dil::En),
        _ => None,
    }
}

/// Ortamın yerel ayarı: `LC_ALL`, `LC_MESSAGES`, `LANG` sırasıyla ilk dolu olan
fn ortam_dili() -> Option<Dil> {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|a| std::env::var(a).ok()).find(|d| !d.trim().is_empty()).and_then(|d| yerel_ayardan(&d))
}

/// Windows dil kimliğinden (LANGID): birincil dil Türkçe (0x1F) → Türkçe; 0 → belirlenemedi; başka → İngilizce
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_dili(langid: u16) -> Option<Dil> {
    match langid & 0x3ff {
        0 => None,
        0x1f => Some(Dil::Tr),
        _ => Some(Dil::En),
    }
}

#[cfg(windows)]
fn sistem_dili() -> Option<Dil> {
    // SAFETY: parametresiz, yalnızca okuyan sistem çağrısı
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    windows_dili(id).or_else(ortam_dili)
}

#[cfg(not(windows))]
fn sistem_dili() -> Option<Dil> {
    ortam_dili()
}
