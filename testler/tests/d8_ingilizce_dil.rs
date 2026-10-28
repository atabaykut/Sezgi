//! D8 İngilizce: çıktı dilinin seçimi. Öncelik: `--lang` bayrağı (her komutta, her konumda) > `SEZGI_LANG` > sistem
//! dili (`LC_ALL`, `LC_MESSAGES`, `LANG`) > Türkçe. Geçersiz `--lang` değeri kullanım hatasıdır (çıkış kodu 2).
use std::process::{Command, Output};

/// Dil ortamı temizlenmiş komut; verilen değişkenler eklenir
fn sezgi(args: &[&str], ortam: &[(&str, &str)]) -> Output {
    let ev = std::env::temp_dir().join(format!("sezgi-d8-dil-{}", std::process::id()));
    std::fs::create_dir_all(&ev).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
    c.args(args)
        .current_dir(&ev)
        .env("HOME", &ev)
        .env("XDG_CONFIG_HOME", ev.join("ayar"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .env_remove("SEZGI_LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG");
    for (a, d) in ortam {
        c.env(a, d);
    }
    c.output().unwrap()
}

fn yardim(args: &[&str], ortam: &[(&str, &str)]) -> String {
    let o = sezgi(args, ortam);
    assert!(o.status.success(), "{args:?} {ortam:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn ingilizce(s: &str) -> bool {
    s.contains("reads your application's code")
}

fn turkce(s: &str) -> bool {
    s.contains("uygulamanızın kodunu okuyup")
}

#[test]
fn varsayilan_turkce() {
    assert!(turkce(&yardim(&["--help"], &[])));
    // Belirlenemeyen yerel ayar Türkçe bırakır
    assert!(turkce(&yardim(&["--help"], &[("LANG", "C.UTF-8")])));
    assert!(turkce(&yardim(&["--help"], &[("LC_ALL", "POSIX")])));
}

#[test]
fn sistem_dili() {
    assert!(ingilizce(&yardim(&["--help"], &[("LANG", "en_US.UTF-8")])));
    // Türkçe dışındaki her açık dil İngilizce
    assert!(ingilizce(&yardim(&["--help"], &[("LANG", "de_DE.UTF-8")])));
    assert!(turkce(&yardim(&["--help"], &[("LANG", "tr_TR.UTF-8")])));
    // LC_ALL > LC_MESSAGES > LANG
    assert!(turkce(&yardim(&["--help"], &[("LC_ALL", "tr_TR.UTF-8"), ("LANG", "en_US.UTF-8")])));
    assert!(ingilizce(&yardim(&["--help"], &[("LC_MESSAGES", "en_GB.UTF-8"), ("LANG", "tr_TR.UTF-8")])));
}

#[test]
fn sezgi_lang_sistem_dilinin_onunde() {
    assert!(ingilizce(&yardim(&["--help"], &[("SEZGI_LANG", "en"), ("LANG", "tr_TR.UTF-8")])));
    assert!(turkce(&yardim(&["--help"], &[("SEZGI_LANG", "tr"), ("LANG", "en_US.UTF-8")])));
    // Tanınmayan SEZGI_LANG sistem diline bırakır
    assert!(ingilizce(&yardim(&["--help"], &[("SEZGI_LANG", "xx"), ("LANG", "en_US.UTF-8")])));
}

#[test]
fn bayrak_her_seyin_onunde_ve_her_konumda() {
    assert!(ingilizce(&yardim(&["--lang", "en", "--help"], &[("SEZGI_LANG", "tr"), ("LANG", "tr_TR.UTF-8")])));
    assert!(turkce(&yardim(&["--help", "--lang", "tr"], &[("SEZGI_LANG", "en")])));
    assert!(ingilizce(&yardim(&["yardim", "--lang=en"], &[])));
    // Eski ad
    assert!(ingilizce(&yardim(&["help", "--dil", "en"], &[])));
    // Komutun bayrak denetimi bayrağı tanır: komutun kendi argümanlarıyla birlikte
    let o = sezgi(&["init", "--lang", "en", "yok-boyle-klasor", "--deneme"], &[]);
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(!e.contains("Unknown flag") && !e.contains("Bilinmeyen bayrak"), "{e}");
    assert!(e.contains("yok-boyle-klasor"), "konumsal argüman bayraktan sonra da okunur: {e}");
}

#[test]
fn gecersiz_deger_kullanim_hatasi() {
    let o = sezgi(&["--lang", "fr", "--help"], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--lang"));
    let o = sezgi(&["rapor", "--lang"], &[]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn bilinmeyen_komut_onerisi_ingilizce() {
    let o = sezgi(&["reprot"], &[("SEZGI_LANG", "en")]);
    assert_eq!(o.status.code(), Some(2));
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("Unknown command: reprot") && e.contains("Did you mean: sezgi report"), "{e}");
    let o = sezgi(&["reprot"], &[]);
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("Bilinmeyen komut: reprot"), "{e}");
}
