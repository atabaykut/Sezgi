//! Monorepo desteği: Uygulamanın üst klasörlerinde çalışma alanı (workspaces) tanımı arar ve
//! uygulamanın package.json'da bağımlı olduğu kardeş paketlerin klasörlerini bulur.
//! Desteklenen tanımlar: package.json "workspaces" (dizi ya da { packages }) ve pnpm-workspace.yaml.

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn json(p: &Path) -> Option<Value> {
    serde_json::from_str(&crate::guvenli::oku(p)?).ok()
}

/// Kök klasörü ve çalışma alanı desenlerini bulur
fn kok_ve_desenler(uygulama: &Path) -> Option<(PathBuf, Vec<String>)> {
    // Uygulama kendi git deposunun köküyse üst klasörlere hiç çıkılmaz
    if uygulama.join(".git").exists() {
        return None;
    }
    let mut d = uygulama.parent();
    while let Some(k) = d {
        if let Some(v) = json(&k.join("package.json")) {
            let w = &v["workspaces"];
            let desenler: Vec<String> =
                w.as_array().or_else(|| w["packages"].as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            if !desenler.is_empty() {
                return Some((k.to_path_buf(), desenler));
            }
        }
        if let Some(y) = crate::guvenli::oku(&k.join("pnpm-workspace.yaml")) {
            let desenler: Vec<String> = y
                .lines()
                .filter_map(|l| l.trim().strip_prefix("- "))
                .map(|s| s.trim().trim_matches(|c| c == '\'' || c == '"').to_string())
                .filter(|s| !s.starts_with('!'))
                .collect();
            if !desenler.is_empty() {
                return Some((k.to_path_buf(), desenler));
            }
        }
        // Deponun kökünü (".git") geçip ebeveyn klasörlere çıkılmaz: başka projeler okunmasın
        if k.join(".git").exists() {
            break;
        }
        d = k.parent();
    }
    None
}

/// Klasör (sembolik bağ çözüldükten sonra) çalışma alanı kökünün içinde mi?
pub fn icinde(kok: &Path, p: &Path) -> bool {
    match (std::fs::canonicalize(kok), std::fs::canonicalize(p)) {
        (Ok(k), Ok(p)) => p.starts_with(k),
        _ => false,
    }
}

/// "packages/*" ya da "packages/platform/*" gibi bir deseni klasörlere açar (yalnızca * desteklenir)
fn ac(kok: &Path, desen: &str) -> Vec<PathBuf> {
    let mut adaylar = vec![kok.to_path_buf()];
    for parca in desen.trim_end_matches('/').split('/') {
        let mut yeni = Vec::new();
        for a in adaylar {
            if parca == "*" || parca == "**" {
                if let Ok(rd) = std::fs::read_dir(&a) {
                    for g in rd.flatten() {
                        if g.path().is_dir() && g.file_name() != "node_modules" {
                            yeni.push(g.path());
                        }
                    }
                }
            } else {
                let p = a.join(parca);
                if p.is_dir() {
                    yeni.push(p);
                }
            }
        }
        adaylar = yeni;
    }
    adaylar
}

/// Uygulamanın bağımlı olduğu kardeş paketler: (paket adı, klasör)
pub fn bagimli_paketler(uygulama: &Path) -> Vec<(String, PathBuf)> {
    let Some(pkg) = json(&uygulama.join("package.json")) else { return vec![] };
    let Some((kok, desenler)) = kok_ve_desenler(uygulama) else { return vec![] };
    let mut adlar: HashMap<String, PathBuf> = HashMap::new();
    for desen in &desenler {
        for k in ac(&kok, desen).into_iter().filter(|k| icinde(&kok, k)) {
            if let Some(v) = json(&k.join("package.json")) {
                if let Some(ad) = v["name"].as_str() {
                    adlar.insert(ad.to_string(), k);
                }
            }
        }
    }
    let kendi = pkg["name"].as_str().unwrap_or("");
    let mut sonuc: Vec<(String, PathBuf)> = Vec::new();
    for alan in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(m) = pkg[alan].as_object() {
            for ad in m.keys() {
                if ad != kendi {
                    if let Some(k) = adlar.get(ad) {
                        if !sonuc.iter().any(|(a, _)| a == ad) && k != uygulama {
                            sonuc.push((ad.clone(), k.clone()));
                        }
                    }
                }
            }
        }
    }
    sonuc.sort();
    sonuc
}
