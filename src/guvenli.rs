//! Güvenilmeyen depodan okuma ve koddan gelen metni temizleme. Sezgi'nin dosya okumaları buradan geçer:
//! sembolik bağ izlenmez, yalnızca düzenli dosya, en fazla 4 MiB.

use std::io::Read;
use std::path::{Path, PathBuf};

/// Tek bir kaynak dosyanın en büyük boyu
pub const EN_BUYUK_DOSYA: u64 = 4 << 20;

/// Sembolik bağ olmayan, düzenli ve 4 MiB'tan küçük bir dosyayı okur; değilse `None`
pub fn oku(p: &Path) -> Option<String> {
    let m = std::fs::symlink_metadata(p).ok()?;
    if !m.file_type().is_file() || m.len() > EN_BUYUK_DOSYA {
        return None;
    }
    let mut s = String::new();
    std::fs::File::open(p).ok()?.take(EN_BUYUK_DOSYA).read_to_string(&mut s).ok()?;
    Some(s)
}

/// Harita dosyası (`.sezgi/` içindeki kapilar.json, benlik.json, openapi.json ...): depoya işlenmiş olabilir. Sembolik
/// bağ izlenmez (ör. `openapi.json -> ~/.ssh/...`), yalnızca düzenli dosya; büyük uygulamaların haritası kaynak dosya
/// sınırını aşabileceği için sınır daha geniştir (64 MiB)
pub fn oku_harita(p: &Path) -> Option<String> {
    const SINIR: u64 = 64 << 20;
    let m = std::fs::symlink_metadata(p).ok()?;
    if !m.file_type().is_file() || m.len() > SINIR {
        return None;
    }
    let mut s = String::new();
    std::fs::File::open(p).ok()?.take(SINIR).read_to_string(&mut s).ok()?;
    Some(s)
}

/// Bayt olarak okur (ayrıştırıcı için)
pub fn oku_bayt(p: &Path) -> Option<Vec<u8>> {
    let m = std::fs::symlink_metadata(p).ok()?;
    if !m.file_type().is_file() || m.len() > EN_BUYUK_DOSYA {
        return None;
    }
    let mut v = Vec::new();
    std::fs::File::open(p).ok()?.take(EN_BUYUK_DOSYA).read_to_end(&mut v).ok()?;
    Some(v)
}

/// Görünmeyen ya da satır/yön değiştiren karakterler. Ajanın gördüğü metinle insanın gördüğü aynı olsun: gizli
/// talimat (ör. Unicode Tag ile "ASCII kaçakçılığı") ya da yön değiştirme ile sahte metin taşınamasın.
/// Unicode genel kategorileriyle: Cc (kontrol), Cf (biçim: sıfır genişlikli, bidi, yumuşak tire, Tag'ler), Zl/Zp
/// (satır/paragraf ayırıcı), Co (özel kullanım), karakter olmayanlar; ayrıca varyasyon seçicileri, birleştirme
/// engelleyici ve Hangul dolgu harfleri. İzin listesi (yalnızca harf, rakam, noktalama, sembol, boşluk) Unicode
/// kategori tablosu gerektirir; bağımlılık eklememek için kategori aralıkları burada açıkça yazıldı (Unicode 16).
fn gorunmez(c: char) -> bool {
    let u = c as u32;
    c.is_control()
        || matches!(
            u,
            // Cf: biçim karakterleri
            0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2 | 0x180E
                | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB
                | 0x110BD | 0x110CD | 0x13430..=0x1345F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
                // Tag'ler (U+E0000–E007F) ve varyasyon seçicileri eki (U+E0100–E01EF): bütün E0000 bloğu
                | 0xE0000..=0xE0FFF
                // Zl, Zp
                | 0x2028 | 0x2029
                // Varyasyon seçicileri, Moğolca serbest varyasyon seçicileri, birleştirme engelleyici
                | 0xFE00..=0xFE0F | 0x180B..=0x180D | 0x180F | 0x034F
                // Hangul ve Khmer dolgu / görünmez harfleri
                | 0x115F | 0x1160 | 0x17B4 | 0x17B5 | 0x3164 | 0xFFA0
                // Co: özel kullanım alanları
                | 0xE000..=0xF8FF | 0xF0000..=0x10FFFF
                // Karakter olmayanlar
                | 0xFDD0..=0xFDEF
        )
        || (u & 0xFFFE) == 0xFFFE
}

/// Koddan gelen metin: görünmez karakterler boşluğa, en fazla 300 karakter (istem enjeksiyonu ve satır sahteciliğine karşı)
pub fn temiz(s: &str) -> String {
    s.chars().map(|c| if gorunmez(c) { ' ' } else { c }).take(300).collect()
}

/// Uygulamadan gelen cevap: görünmez karakterler boşluğa (satır sonu ve sekme kalır), en fazla `n` karakter. Cevap
/// kaynak koddan değil çalışan uygulamadan gelir ama onun verisi de (ör. bir kullanıcının yazdığı yorum) talimat taşıyabilir.
pub fn cevap_temiz(s: &str, n: usize) -> String {
    s.chars().map(|c| if c != '\n' && c != '\t' && gorunmez(c) { ' ' } else { c }).take(n).collect()
}

/// Bir JSON değerindeki bütün metinleri ve anahtarları temizler (MCP ve Ayna haritayı yüklerken)
pub fn json_temiz(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::String(s) => {
            if s.chars().any(gorunmez) || s.chars().count() > 300 {
                *s = temiz(s);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(json_temiz),
        Value::Object(m) => {
            let eski = std::mem::take(m);
            for (k, mut x) in eski {
                json_temiz(&mut x);
                m.insert(temiz(&k), x);
            }
        }
        _ => {}
    }
}

/// Kökten göreli yol, "/" ayraçlı. Kökün dışındaki dosya (kardeş paket) "../../packages/x" biçiminde yazılır.
/// Mutlak yol (kullanıcı adı, makine düzeni) hiçbir çıktıya yazılmaz: ortak kök yoksa yalnızca dosya adı.
pub fn goreli(kok: &Path, p: &Path) -> String {
    if let Ok(g) = p.strip_prefix(kok) {
        return g.to_string_lossy().replace('\\', "/");
    }
    let k = std::fs::canonicalize(kok).unwrap_or_else(|_| kok.to_path_buf());
    let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (a, b): (Vec<_>, Vec<_>) = (k.components().collect(), p.components().collect());
    let ortak = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if ortak == 0 {
        return format!("<dış>/{}", p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    }
    let mut parcalar: Vec<String> = vec!["..".into(); a.len() - ortak];
    parcalar.extend(b[ortak..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    parcalar.join("/")
}

/// Ekranda gösterilecek yol: Windows'un uzun yol öneki (`\\?\`) atılır
pub fn gorunen(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(String::from).unwrap_or(s)
}

/// Dosyayı önce yanındaki geçici dosyaya yazar, sonra tek adımda yerine taşır: okuyan taraf (MCP sunucusu, Ayna)
/// eski ya da yeni hâli görür, yarım dosyayı hiçbir zaman görmez
pub fn atomik_yaz(yol: &Path, icerik: &[u8]) -> Result<(), String> {
    atomik_yaz_kipli(yol, icerik, false)
}

/// [`atomik_yaz`], ama dosya yalnızca kullanıcıya açık (Unix'te 0600) oluşur: kullanıcının yapılandırma klasöründeki
/// canlı mod durumu (token kayıtları, ayar, kayıt, onay kuyruğu, hafıza) için
pub fn atomik_yaz_gizli(yol: &Path, icerik: &[u8]) -> Result<(), String> {
    atomik_yaz_kipli(yol, icerik, true)
}

/// Kullanıcıya özel dosya açma seçenekleri (Unix'te 0600; Windows'ta kullanıcı profilinin izinleri geçerlidir)
pub fn gizli_secenekler() -> std::fs::OpenOptions {
    #[allow(unused_mut)]
    let mut o = std::fs::OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o
}

/// Kullanıcıya özel klasör: yoksa oluşturulur; Unix'te kendisi (ve oluşturulan üst klasörler) 0700
pub fn gizli_klasor(p: &Path) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        b.mode(0o700);
        b.create(p)?;
        if std::fs::metadata(p)?.permissions().mode() & 0o777 != 0o700 {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    b.create(p)
}

/// Önceki sürümlerin varsayılan izinlerle (ör. 0755 / 0644) yazdığı bir klasörü yalnızca kullanıcıya açık yapar:
/// klasörler 0700, dosyalar 0600 (sembolik bağ izlenmez). Windows'ta bir şey yapmaz.
pub fn izinleri_daralt(p: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(m) = std::fs::symlink_metadata(p) else { return };
        if m.file_type().is_symlink() {
            return;
        }
        let kip = if m.is_dir() { 0o700 } else { 0o600 };
        if m.permissions().mode() & 0o777 != kip {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(kip));
        }
        if m.is_dir() {
            for g in std::fs::read_dir(p).into_iter().flatten().flatten() {
                izinleri_daralt(&g.path());
            }
        }
    }
    #[cfg(not(unix))]
    let _ = p;
}

fn atomik_yaz_kipli(yol: &Path, icerik: &[u8], gizli: bool) -> Result<(), String> {
    let ad = yol.file_name().map(|a| a.to_string_lossy().to_string()).unwrap_or_default();
    let gecici = yol.with_file_name(format!(".{ad}.{}.sezgi-gecici", std::process::id()));
    if gizli {
        // Geçici dosya baştan 0600 açılır (yazılıp sonra daraltılırsa arada okunabilir kalırdı)
        let _ = std::fs::remove_file(&gecici);
        gizli_secenekler()
            .write(true)
            .create_new(true)
            .open(&gecici)
            .and_then(|mut f| std::io::Write::write_all(&mut f, icerik))
            .map_err(|e| tf!("{} yazılamadı: {e}", "could not write {}: {e}", yol.display()))?;
    } else {
        std::fs::write(&gecici, icerik).map_err(|e| tf!("{} yazılamadı: {e}", "could not write {}: {e}", yol.display()))?;
    }
    // Windows'ta okuyan taraf dosyayı o an açık tutuyorsa yer değiştirme kısa süre reddedilebilir: birkaç kez denenir
    let mut son = None;
    for _ in 0..20 {
        match std::fs::rename(&gecici, yol) {
            Ok(()) => return Ok(()),
            Err(e) => {
                son = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }
    let _ = std::fs::remove_file(&gecici);
    Err(tf!("{} yazılamadı: {}", "could not write {}: {}", yol.display(), son.map(|e| e.to_string()).unwrap_or_default()))
}

/// Haritanın klasörü: verildiyse o; verilmediyse bulunulan klasörde ve üstlerinde (en çok 8 kat, git kökünde durur)
/// `.sezgi/kapilar.json` aranır; bulunamazsa `.sezgi`
pub fn harita_bul(verilen: Option<&str>) -> PathBuf {
    if let Some(v) = verilen {
        return PathBuf::from(v);
    }
    if let Ok(c) = std::env::current_dir() {
        for k in c.ancestors().take(8) {
            if k.join(".sezgi").join("kapilar.json").is_file() {
                return k.join(".sezgi");
            }
            if k.join(".git").exists() {
                break;
            }
        }
    }
    PathBuf::from(".sezgi")
}

/// [`harita_bul`]; harita yoksa ne yapılacağını söyleyip çıkar
pub fn harita_gerekli(verilen: Option<&str>) -> PathBuf {
    let h = harita_bul(verilen);
    if !h.join("kapilar.json").is_file() {
        match verilen {
            Some(v) => te!("✗ {v} içinde harita yok. Önce: sezgi init", "✗ No map in {v}. First: sezgi init"),
            None => te!(
                "✗ Bu klasörde ve üst klasörlerde harita yok. Önce uygulamanın klasöründe: sezgi init",
                "✗ No map in this folder or the folders above it. First, in the application folder: sezgi init"
            ),
        }
        std::process::exit(1);
    }
    h
}
