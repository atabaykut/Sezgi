//! Gerçek veride yazmanın saf kuralları: saatlik yazma sınırı, insanın terminaldeki onay cevapları ve gerçek-veri
//! token'ı. Terminal, dosya ve ağ işleri canlı modda (`canli`); burada yalnızca karar verilir.
use serde_json::Value;

/// Son bir saatteki yazma zamanları sınırı doldurdu mu; dolmadıysa pencere içindeki yazma sayısı
pub fn hiz_siniri(zamanlar: &[u64], simdi: u64, sinir: u64) -> Result<usize, String> {
    let n = zamanlar.iter().filter(|z| simdi.saturating_sub(**z) < 3600).count();
    if n as u64 >= sinir {
        return Err(tf!(
            "GERÇEK VERİ: saatlik yazma sınırı doldu (son bir saatte {n} yazma, sınır {sinir}); bir süre sonra yeniden deneyin. Sınırı yalnızca terminalde değiştirebilirsiniz: sezgi hedef --saatlik-yazma <1-200>",
            "REAL DATA: hourly write limit reached ({n} writes in the last hour, limit {sinir}); try again later. Only a terminal can change the limit: sezgi target --saatlik-yazma <1-200>"
        ));
    }
    Ok(n)
}

/// Gerçek veride onay cevabı: yıkıcı işlemde insan yöntemi ve yolu aynen yazar (ör. `DELETE /api/kitaplar/7`), diğer
/// yazmalarda açık "evet" (İngilizce çıktıda "yes" de; kısaltma kabul edilmez)
pub fn gercek_onay_cevabi(risk_: &str, yontem: &str, yol: &str, cevap: &str) -> bool {
    let cevap = cevap.trim();
    if risk_ == "yikici" {
        let beklenen = format!("{} {yol}", yontem.to_uppercase());
        cevap.split_whitespace().collect::<Vec<_>>().join(" ") == beklenen
    } else {
        cevap == "evet" || (crate::dil::en() && cevap == "yes")
    }
}

/// Token gerçek veride yazabilir mi: terminalde `--gercek-veri` ile üretilmiş olmalı
pub fn token_gercek_veri(token: &Value) -> Result<(), String> {
    if token["gercek_veri"] == true {
        return Ok(());
    }
    Err(tf!(
        "token '{}' gerçek veride yalnızca okur: gerçek veride yazmak için token terminalde --gercek-veri ile üretilmeli (sezgi token olustur --ad <ad> --izin <desen:seviye> --gercek-veri)",
        "token '{}' only reads on real data: to write on real data the token must be created in a terminal with --gercek-veri (sezgi token create --name <name> --izin <pattern:level> --gercek-veri)",
        token["ad"].as_str().unwrap_or("")
    ))
}

/// Gerçek veriyi açma cevabı: uygulamanın adı aynen (baştaki ve sondaki boşluk hariç)
pub fn gercek_ac_cevabi(ad: &str, cevap: &str) -> bool {
    !ad.trim().is_empty() && cevap.trim() == ad.trim()
}
