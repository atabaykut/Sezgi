//! Sürümler arası fark: `sezgi karsilastir` ve Benlik Kartı `changes` (SPEC §7). Bağlı yapay zekâları bozacak
//! değişiklikler (yeni zorunlu alan, kaldırılan alan, daralan değerler, sıkılaşan sınırlar, giriş istenmesi, kaldırılan
//! işlem) işaretlenir; bozmayanlar (yeni isteğe bağlı alan, yeni işlem) işaretlenmez.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

fn hazirla(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-fark-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &k);
    k
}

fn sezgi(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sezgi")).args(args).output().unwrap()
}

fn degistir(k: &Path, dosya: &str, eski: &str, yeni: &str) {
    let f = k.join(dosya);
    // Windows'ta depo CRLF ile açılmış olabilir
    let m = std::fs::read_to_string(&f).unwrap().replace("\r\n", "\n");
    assert!(m.contains(eski), "{dosya}: {eski}");
    std::fs::write(&f, m.replacen(eski, yeni, 1)).unwrap();
}

/// Bozan ve bozmayan değişikliklerin hepsi
fn degisiklikler(k: &Path) {
    let dto = "src/kitap/dto/kitap.dto.ts";
    let ctrl = "src/kitap/kitap.controller.ts";
    degistir(k, dto, "@IsOptional() @IsString() yazar?: string;", "@IsString() yazar!: string;");
    degistir(k, dto, "@IsIn(['tr', 'en']) dil", "@IsIn(['tr']) dil");
    degistir(k, dto, "@Min(1) @Max(5000) sayfa", "@Min(10) @Max(5000) sayfa");
    degistir(k, dto, "  @ValidateIf(", "  @IsOptional() @IsString() altBaslik?: string;\n  @ValidateIf(");
    degistir(k, ctrl, "  @Public()\n  @Get()", "  @Get()");
    degistir(k, ctrl, "  @Roles('ADMIN')\n  @Delete(':id')\n  sil(@Param('id') id: string) {\n    return this.kitaplar.sil(id);\n  }\n", "");
    degistir(k, ctrl, "  @Get([':id', ':id/detay'])", "  @Get('yeni/uc')\n  yeniUc() {}\n\n  @Get([':id', ':id/detay'])");
}

fn json(o: &Output) -> serde_json::Value {
    assert!(!o.stdout.is_empty(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn sebepler(v: &serde_json::Value) -> String {
    v["breaking"].as_array().unwrap().iter().map(|b| format!("{}: {}", b["operation"].as_str().unwrap(), b["reason"].as_str().unwrap())).collect::<Vec<_>>().join("\n")
}

#[test]
fn karsilastir_bozan_degisiklikleri_bulur() {
    let k = hazirla("karsilastir");
    let ks = k.to_str().unwrap();
    let eski = k.with_extension("eski");
    let _ = std::fs::remove_dir_all(&eski);
    assert!(sezgi(&["init", ks, "--cikti", eski.to_str().unwrap()]).status.success());
    degisiklikler(&k);
    assert!(sezgi(&["init", ks]).status.success());
    let yeni = k.join(".sezgi");
    let v = json(&sezgi(&["karsilastir", eski.to_str().unwrap(), yeni.to_str().unwrap(), "--json"]));
    let s = sebepler(&v);
    assert!(v["added"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("yeni")), "{v}");
    assert!(v["removed"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("sil")), "{v}");
    assert!(s.contains("`yazar` is now required"), "{s}");
    assert!(s.contains("`dil` no longer accepts: en"), "{s}");
    assert!(s.contains("`sayfa` minimum raised from 1 to 10"), "{s}");
    assert!(s.contains("login is now required"), "{s}");
    // Yeni isteğe bağlı alan bozmaz
    assert!(!s.contains("altBaslik"), "{s}");
    assert!(v["non_breaking"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("altBaslik")), "{v}");
    // PATCH (PartialType) alanları isteğe bağlı kalır: bozmaz
    assert!(!s.lines().any(|l| l.contains("guncelle") && l.contains("yazar")), "{s}");
    // --sert: bozan değişiklik varsa başarısız; aynı harita kendisiyle karşılaştırılınca başarılı
    assert_eq!(sezgi(&["karsilastir", eski.to_str().unwrap(), yeni.to_str().unwrap(), "--sert"]).status.code(), Some(1));
    let ayni = sezgi(&["karsilastir", yeni.to_str().unwrap(), yeni.to_str().unwrap(), "--sert"]);
    assert!(ayni.status.success(), "{}", String::from_utf8_lossy(&ayni.stdout));
    // İnsan çıktısı
    let m = String::from_utf8_lossy(&sezgi(&["karsilastir", eski.to_str().unwrap(), yeni.to_str().unwrap()]).stdout).to_string();
    assert!(m.contains("Bozan") && m.contains("Kaldırılan"), "{m}");
    let _ = std::fs::remove_dir_all(&k);
    let _ = std::fs::remove_dir_all(&eski);
}

fn git(k: &Path, args: &[&str]) {
    let c = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]).args(args).current_dir(k).output().unwrap();
    assert!(c.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&c.stderr));
}

fn kart(k: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(k.join(".sezgi").join("benlik.json")).unwrap()).unwrap()
}

#[test]
fn init_yeni_committe_changes_yazar() {
    let k = hazirla("changes");
    let ks = k.to_str().unwrap();
    git(&k, &["init", "-q"]);
    git(&k, &["add", "."]);
    git(&k, &["commit", "-q", "-m", "ilk"]);
    assert!(sezgi(&["init", ks]).status.success());
    let ilk = kart(&k);
    assert!(ilk.get("changes").is_none(), "önceki kart yokken changes yazılmaz");
    let ilk_ref = ilk["app"]["source_ref"].as_str().unwrap().to_string();
    degisiklikler(&k);
    git(&k, &["commit", "-q", "-am", "ikinci"]);
    assert!(sezgi(&["init", ks]).status.success());
    let c = kart(&k)["changes"].clone();
    assert_eq!(c["previous_source_ref"], ilk_ref.as_str());
    assert!(sebepler(&c).contains("`yazar` is now required"), "{c}");
    // Kartta yalnızca şemadaki alanlar (additionalProperties: false)
    let mut anahtarlar: Vec<&String> = c.as_object().unwrap().keys().collect();
    anahtarlar.sort();
    assert_eq!(anahtarlar, ["added", "breaking", "previous_source_ref", "removed"]);
    // Aynı commit'te yeniden çalıştırma farkı silmez
    assert!(sezgi(&["init", ks]).status.success());
    assert_eq!(kart(&k)["changes"], c);
    let _ = std::fs::remove_dir_all(&k);
}
