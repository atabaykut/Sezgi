//! W2-android / P1.8: `karsilastir --sert` GitHub eylemi (`action.yml` + `eylem/karsilastir.sh`). PR'da taban commit'in
//! ve PR'ın haritası kurulur, karşılaştırılır; bozan değişiklikte iş başarısız olur, özet adım özetine ve PR yorumuna
//! (tek yorum, güncellenerek) yazılır. Eylemde yalnızca SHA sabitli `uses:` olur; girdiler betiğe ortam değişkeniyle
//! geçer (iş akışı ifadesi kabuk koduna gömülmez).
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            kopyala(&p, &hedef.join(g.file_name()));
        } else {
            std::fs::copy(&p, hedef.join(g.file_name())).unwrap();
        }
    }
}

fn kaynak() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn git(k: &Path, args: &[&str]) -> String {
    let c = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]).args(args).current_dir(k).output().unwrap();
    assert!(c.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&c.stderr));
    String::from_utf8_lossy(&c.stdout).trim().to_string()
}

fn degistir(f: &Path, eski: &str, yeni: &str) {
    let m = std::fs::read_to_string(f).unwrap().replace("\r\n", "\n");
    assert!(m.contains(eski), "{}: {eski}", f.display());
    std::fs::write(f, m.replacen(eski, yeni, 1)).unwrap();
}

struct Depo {
    kok: PathBuf,
    taban: String,
}

impl Drop for Depo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

/// Uygulama `api/` altında; ilk commit taban, ikinci commit PR'ın değişikliği
fn depo(ad: &str, degisiklik: impl Fn(&Path)) -> Depo {
    let kok = std::env::temp_dir().join(format!("sezgi-w2a-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&kaynak().join("tests").join("ornekler").join("nestjs-temel"), &kok.join("api"));
    std::fs::create_dir_all(kok.join("ev")).unwrap();
    std::fs::create_dir_all(kok.join("bin")).unwrap();
    git(&kok, &["init", "-q"]);
    std::fs::write(kok.join(".gitignore"), "ev/\nbin/\ncikti/\n").unwrap();
    git(&kok, &["add", "."]);
    git(&kok, &["commit", "-q", "-m", "taban"]);
    let taban = git(&kok, &["rev-parse", "HEAD"]);
    degisiklik(&kok.join("api"));
    git(&kok, &["commit", "-q", "-am", "pr"]);
    // Sahte gh: çağrıları kaydeder; var olan yorum yok
    let gh = kok.join("bin").join("gh");
    std::fs::write(&gh, "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> \"$GH_KAYIT\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    Depo { kok, taban }
}

struct Sonuc {
    cikis: Output,
    ozet: String,
    cikti: String,
    gh: String,
}

fn eylem(d: &Depo, sert: &str, pr: &str) -> Sonuc {
    let (ozet, cikti, gh) = (d.kok.join("ozet.md"), d.kok.join("cikti.txt"), d.kok.join("gh.txt"));
    let yol = format!("{}:{}", d.kok.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    let cikis = Command::new("bash")
        .arg(kaynak().join("eylem").join("karsilastir.sh"))
        .current_dir(&d.kok)
        .env("PATH", yol)
        .env("HOME", d.kok.join("ev"))
        .env("SEZGI_IKILI", env!("CARGO_BIN_EXE_sezgi"))
        .env("SEZGI_UYGULAMA", "api")
        .env("SEZGI_TABAN", &d.taban)
        .env("SEZGI_SERT", sert)
        .env("SEZGI_YORUM", "true")
        .env("SEZGI_PR", pr)
        .env("GH_TOKEN", "sahte")
        .env("GITHUB_REPOSITORY", "ornek/uygulama")
        .env("GITHUB_STEP_SUMMARY", &ozet)
        .env("GITHUB_OUTPUT", &cikti)
        .env("GH_KAYIT", &gh)
        .output()
        .unwrap();
    let oku = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    Sonuc { ozet: oku(&ozet), cikti: oku(&cikti), gh: oku(&gh), cikis }
}

fn metin(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn uc_kaldir(api: &Path) {
    degistir(&api.join("src/kitap/kitap.controller.ts"), "  @Roles('ADMIN')\n  @Delete(':id')\n  sil(@Param('id') id: string) {\n    return this.kitaplar.sil(id);\n  }\n", "");
}

#[test]
fn w2a_bozan_degisiklikte_basarisiz_ve_pr_yorumu() {
    let d = depo("bozan", uc_kaldir);
    let s = eylem(&d, "true", "12");
    assert_eq!(s.cikis.status.code(), Some(1), "{}", metin(&s.cikis));
    assert!(s.cikti.contains("kirilma=true"), "{}", s.cikti);
    assert!(s.ozet.contains("Kaldırılan") && s.ozet.contains("sil"), "{}", s.ozet);
    assert!(s.ozet.contains("<!-- sezgi-karsilastir -->"), "{}", s.ozet);
    // Var olan yorum aranır, yoksa yenisi yazılır
    assert!(s.gh.contains("repos/ornek/uygulama/issues/12/comments"), "{}", s.gh);
    assert!(s.gh.lines().any(|l| l.contains("POST") && l.contains("issues/12/comments")), "{}", s.gh);
    // Taban çalışma ağacı temizlendi; depoya bir şey yazılmadı
    assert_eq!(git(&d.kok, &["worktree", "list"]).lines().count(), 1);
    assert_eq!(git(&d.kok, &["status", "--porcelain", "--untracked-files=all", "api"]), "");
}

#[test]
fn w2a_sert_kapaliyken_raporlar_ama_gecer() {
    let d = depo("yumusak", uc_kaldir);
    let s = eylem(&d, "false", "");
    assert!(s.cikis.status.success(), "{}", metin(&s.cikis));
    assert!(s.cikti.contains("kirilma=true"), "{}", s.cikti);
    // PR numarası yok (PR dışı olay): yorum yazılmaz
    assert_eq!(s.gh, "");
}

#[test]
fn w2a_bozmayan_degisiklik_gecer() {
    let d = depo("bozmayan", |api| {
        degistir(&api.join("src/kitap/kitap.controller.ts"), "  @Get([':id', ':id/detay'])", "  @Get('yeni/uc')\n  yeniUc() {}\n\n  @Get([':id', ':id/detay'])")
    });
    let s = eylem(&d, "true", "12");
    assert!(s.cikis.status.success(), "{}", metin(&s.cikis));
    assert!(s.cikti.contains("kirilma=false"), "{}", s.cikti);
    assert!(s.ozet.contains("Eklenen"), "{}", s.ozet);
}

#[test]
fn w2a_eylem_tanimi_sabitli_ve_enjeksiyonsuz() {
    let eylem = std::fs::read_to_string(kaynak().join("action.yml")).unwrap();
    let ornek = std::fs::read_to_string(kaynak().join("eylem").join("ornek-is-akisi.yml")).unwrap();
    assert!(eylem.contains("using: composite"), "{eylem}");
    for (ad, m) in [("action.yml", &eylem), ("ornek-is-akisi.yml", &ornek)] {
        // Sezgi eyleminin kendisi örnekte yer tutucuyla durur (kullanıcı sürümün commit SHA'sını yazar); diğerleri sabitli
        for l in m.lines().filter(|l| l.trim_start().trim_start_matches("- ").starts_with("uses:") && !l.contains("atabaykut/Sezgi@")) {
            let r = l.split('@').nth(1).unwrap_or("").split_whitespace().next().unwrap_or("");
            assert!(r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit()), "{ad}: SHA sabitli değil: {l}");
        }
    }
    // `run:` satırlarında iş akışı ifadesi yok: girdiler ortam değişkeniyle geçer
    let mut run_icinde = false;
    for l in eylem.lines() {
        let t = l.trim_start();
        if t.starts_with("run:") || t.starts_with("- run:") {
            run_icinde = true;
        } else if t.starts_with("- ") || t.starts_with("env:") || t.starts_with("shell:") || t.starts_with("id:") || t.starts_with("name:") {
            run_icinde = false;
        }
        assert!(!(run_icinde && t.contains("${{")), "run içinde ifade: {l}");
    }
}
