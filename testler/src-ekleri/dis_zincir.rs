// dis_zincir.rs birim testleri: paket sınıflandırması genel kalıptır (dil ayraçları, takma ad, yöntem adına göre Go
// standart kütüphanesi), bilinmeyen paket bilinmiyor kalır
use super::*;

#[test]
fn dis_paketler_tur_ve_eslesen_onekle() {
    assert_eq!(sinifla("pycurl", "perform"), Sinif::Dis("http", "pycurl"));
    assert_eq!(sinifla("django.core.mail.EmailMultiAlternatives", "send"), Sinif::Dis("eposta", "django.core.mail"));
    assert_eq!(sinifla("@sendgrid/mail", "send"), Sinif::Dis("eposta", "sendgrid"));
    assert_eq!(sinifla("org.springframework.web.client.RestTemplate", "postForObject"), Sinif::Dis("http", "org.springframework.web.client"));
    assert_eq!(sinifla("php:GuzzleHttp\\Client", "post"), Sinif::Dis("http", "guzzlehttp"));
    assert_eq!(sinifla("php:Illuminate\\Support\\Facades\\Mail", "to"), Sinif::Dis("eposta", "illuminate.support.facades.mail"));
    assert_eq!(sinifla("rb:Stripe::Charge", "create"), Sinif::Dis("odeme", "stripe"));
    assert_eq!(sinifla("@aws-sdk/client-s3", "send"), Sinif::Dis("bulut", "aws-sdk"));
    // Go: aynı pakette istemci dış, sunucu yardımcıları iç
    assert_eq!(sinifla("net/http", "Post"), Sinif::Dis("http", "net.http"));
    assert_eq!(sinifla("net/http", "Error"), Sinif::Ic);
    assert_eq!(sinifla("encoding/json", "Marshal"), Sinif::Ic);
    // Node: sunucu kurmak iç, istek göndermek dış
    assert_eq!(sinifla("node:https", "request"), Sinif::Dis("http", "node:http"));
    assert_eq!(sinifla("http", "createServer"), Sinif::Ic);
}

#[test]
fn ic_paketler_ve_bilinmeyenler() {
    for (m, a) in [("django.db.models", "save"), ("json", "dumps"), ("logging", "getLogger"), ("typeorm", "getRepository"), ("java.util.List", "of"), ("php:ZipArchive", "open"), ("rb:Rails", "logger"), ("rb:BenimModulum", "yap"), ("acme.errors", "NotFoundException"), ("threading.Thread", "start")] {
        assert_eq!(sinifla(m, a), Sinif::Ic, "{m}.{a}");
    }
    // Sınıflandırılamayan kütüphane ve süreç başlatma: bilinmiyor (yazmada onay)
    for (m, a) in [("grafikcizer", "yayinla"), ("subprocess", "call"), ("child_process", "fork"), ("os", "system"), ("php:Acme\\Pdf\\Ciz", "ciz"), ("os/exec", "Command")] {
        assert_eq!(sinifla(m, a), Sinif::Bilinmiyor, "{m}.{a}");
    }
    // Önek parça sınırında eşleşir: "requestsx" requests değildir
    assert_eq!(sinifla("requestsx", "get"), Sinif::Bilinmiyor);
}
