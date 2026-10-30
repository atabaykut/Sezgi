//! Haritanın kapsamı: Sezgi'nin henüz haritalamadığı API türleri (GraphQL, gRPC, tRPC, WebSocket, desteklenmeyen
//! yönlendirici kütüphaneleri) bağımlılık dosyalarından tanınır ve kod büyüklüğüne göre çok az işlem bulunduysa kapsam
//! "düşük" sayılır. İkisi de `init`, `rapor` ve MCP `kimsin` çıktısında söylenir: az bulunan bir harita, uygulamanın
//! tamamını görüyormuş gibi sunulmaz.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// (tür, bağımlılık dosyası türü, aranan ad). Ad, dosyada kendi başına geçmeli (JSON'da tırnaklı anahtar; satır tabanlı
/// dosyalarda satır başında ya da tırnak içinde)
const DESENLER: &[(&str, Dosya, &str)] = &[
    // GraphQL
    ("GraphQL", Dosya::Npm, "@nestjs/graphql"),
    ("GraphQL", Dosya::Npm, "@apollo/server"),
    ("GraphQL", Dosya::Npm, "apollo-server"),
    ("GraphQL", Dosya::Npm, "apollo-server-express"),
    ("GraphQL", Dosya::Npm, "graphql-yoga"),
    ("GraphQL", Dosya::Npm, "type-graphql"),
    ("GraphQL", Dosya::Npm, "mercurius"),
    ("GraphQL", Dosya::Npm, "express-graphql"),
    ("GraphQL", Dosya::Npm, "graphql-http"),
    ("GraphQL", Dosya::Py, "strawberry-graphql"),
    ("GraphQL", Dosya::Py, "graphene"),
    ("GraphQL", Dosya::Py, "graphene-django"),
    ("GraphQL", Dosya::Py, "ariadne"),
    ("GraphQL", Dosya::Gem, "graphql"),
    ("GraphQL", Dosya::Nuget, "HotChocolate"),
    ("GraphQL", Dosya::Nuget, "GraphQL.Server"),
    ("GraphQL", Dosya::Go, "github.com/99designs/gqlgen"),
    ("GraphQL", Dosya::Go, "github.com/graph-gophers/graphql-go"),
    ("GraphQL", Dosya::Go, "github.com/graphql-go/graphql"),
    ("GraphQL", Dosya::Composer, "nuwave/lighthouse"),
    ("GraphQL", Dosya::Composer, "rebing/graphql-laravel"),
    ("GraphQL", Dosya::Jvm, "spring-boot-starter-graphql"),
    ("GraphQL", Dosya::Jvm, "com.graphql-java"),
    ("GraphQL", Dosya::Jvm, "graphql-kotlin"),
    // gRPC
    ("gRPC", Dosya::Npm, "@grpc/grpc-js"),
    ("gRPC", Dosya::Py, "grpcio"),
    ("gRPC", Dosya::Nuget, "Grpc.AspNetCore"),
    ("gRPC", Dosya::Go, "google.golang.org/grpc"),
    ("gRPC", Dosya::Jvm, "grpc-spring-boot-starter"),
    ("gRPC", Dosya::Jvm, "io.grpc"),
    ("gRPC", Dosya::Composer, "spiral/roadrunner-grpc"),
    // tRPC
    ("tRPC", Dosya::Npm, "@trpc/server"),
    // WebSocket
    ("WebSocket", Dosya::Npm, "@nestjs/websockets"),
    ("WebSocket", Dosya::Npm, "socket.io"),
    ("WebSocket", Dosya::Npm, "ws"),
    ("WebSocket", Dosya::Py, "channels"),
    ("WebSocket", Dosya::Go, "github.com/gorilla/websocket"),
    ("WebSocket", Dosya::Go, "nhooyr.io/websocket"),
    ("WebSocket", Dosya::Composer, "laravel/reverb"),
    ("WebSocket", Dosya::Composer, "beyondcode/laravel-websockets"),
    ("WebSocket", Dosya::Jvm, "spring-boot-starter-websocket"),
    // Sezgi'nin okumadığı yönlendirici kütüphaneleri
    ("FastEndpoints", Dosya::Nuget, "FastEndpoints"),
    ("Carter", Dosya::Nuget, "Carter"),
];

#[derive(Clone, Copy, PartialEq)]
enum Dosya {
    Npm,
    Py,
    Gem,
    Nuget,
    Go,
    Composer,
    Jvm,
}

/// Uygulama kökündeki (ve .NET'te dört kata kadar alttaki) bağımlılık dosyaları ve türleri
fn dosyalar(kok: &Path) -> Vec<(Dosya, PathBuf)> {
    let mut v: Vec<(Dosya, PathBuf)> = [
        (Dosya::Npm, "package.json"),
        (Dosya::Py, "requirements.txt"),
        (Dosya::Py, "pyproject.toml"),
        (Dosya::Py, "Pipfile"),
        (Dosya::Py, "setup.py"),
        (Dosya::Gem, "Gemfile"),
        (Dosya::Go, "go.mod"),
        (Dosya::Composer, "composer.json"),
        (Dosya::Jvm, "pom.xml"),
        (Dosya::Jvm, "build.gradle"),
        (Dosya::Jvm, "build.gradle.kts"),
        (Dosya::Jvm, "gradle/libs.versions.toml"),
    ]
    .iter()
    .map(|(t, d)| (*t, kok.join(d)))
    .filter(|(_, p)| p.is_file())
    .collect();
    // requirements/*.txt ve requirements-*.txt
    for g in std::fs::read_dir(kok).into_iter().flatten().flatten() {
        let ad = g.file_name().to_string_lossy().to_string();
        if ad.starts_with("requirements") && ad.ends_with(".txt") && ad != "requirements.txt" {
            v.push((Dosya::Py, g.path()));
        }
    }
    let mut yigin = vec![(kok.to_path_buf(), 0)];
    while let Some((k, d)) = yigin.pop() {
        for g in std::fs::read_dir(&k).into_iter().flatten().flatten() {
            let p = g.path();
            let ad = g.file_name().to_string_lossy().to_string();
            if g.file_type().is_ok_and(|t| t.is_dir()) {
                if d < 4 && !ad.starts_with('.') && !matches!(ad.as_str(), "node_modules" | "bin" | "obj" | "vendor" | "target" | "dist") {
                    yigin.push((p, d + 1));
                }
            } else if ad.ends_with(".csproj") {
                v.push((Dosya::Nuget, p));
            }
        }
    }
    v
}

/// Ad bağımlılık dosyasında kendi başına geçiyor mu
fn geciyor(tur: Dosya, metin: &str, ad: &str) -> bool {
    let ad_l = ad.to_ascii_lowercase();
    match tur {
        Dosya::Npm | Dosya::Composer => metin.contains(&format!("\"{ad}\"")),
        Dosya::Nuget => metin.contains(&format!("Include=\"{ad}\"")) || metin.contains(&format!("Include=\"{ad}.")),
        Dosya::Gem => metin.lines().any(|l| {
            let l = l.trim();
            l.starts_with("gem ") && (l.contains(&format!("'{ad}'")) || l.contains(&format!("\"{ad}\"")))
        }),
        Dosya::Py => metin.lines().any(|l| {
            let l = l.trim().trim_start_matches(['"', '\'']).to_ascii_lowercase();
            l.strip_prefix(&ad_l).is_some_and(|r| r.is_empty() || !r.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        }),
        Dosya::Go => metin.lines().any(|l| l.trim().trim_start_matches("require").trim().starts_with(&format!("{ad} ")) || l.trim() == ad),
        Dosya::Jvm => metin.contains(ad),
    }
}

/// Uygulamada bulunan, Sezgi'nin haritalamadığı API türleri: [{tur, kanit}] (kanıt: bağımlılık adı ve dosyası)
pub fn kapsam_disi(kok: &Path) -> Vec<Value> {
    let mut v: Vec<Value> = vec![];
    for (tur, yol) in dosyalar(kok) {
        let Some(metin) = crate::guvenli::oku(&yol) else { continue };
        let goreli = crate::guvenli::goreli(kok, &yol);
        for (api, t, ad) in DESENLER {
            if *t == tur && geciyor(tur, &metin, ad) && !v.iter().any(|x| x["tur"] == *api) {
                v.push(json!({ "tur": api, "kanit": format!("{ad} ({goreli})") }));
            }
        }
    }
    v
}

/// Kaynak kodda rota kaydına benzeyen ifade sayısı (test dosyaları ve bağımlılık klasörleri hariç). Kesin değil, yalnızca
/// "Sezgi'nin bulduğu işlem sayısı koddaki kayıtların çok altında mı" sorusu için: kendi yönlendiricisi, yansımayla
/// kurulan gruplar ya da Sezgi'nin tanımadığı bir kütüphane bu farkla görünür. JavaScript'te yalnızca sunucu tarafı
/// alıcılar sayılır (`app.get('/x')`, `router.post(...)`): ön yüzün `axios.get('/api/x')` çağrısı rota değildir.
pub fn rota_benzeri(kok: &Path) -> usize {
    static DESEN: std::sync::OnceLock<Vec<(&'static [&'static str], regex::Regex)>> = std::sync::OnceLock::new();
    let desenler = DESEN.get_or_init(|| {
        let r = |s: &str| regex::Regex::new(s).unwrap();
        vec![
            (&["cs"][..], r(r#"\.Map(Get|Post|Put|Patch|Delete|Methods)\(|\[Http(Get|Post|Put|Patch|Delete)\b"#)),
            (&["java", "kt"][..], r(r#"@(Get|Post|Put|Patch|Delete|Request)Mapping\b"#)),
            (&["ts", "js", "mjs", "cjs"][..], r(r#"@(Get|Post|Put|Patch|Delete|All)\(|\b(app|router|routes?|server|fastify|api)\.(get|post|put|patch|delete|all)\(\s*['"`]/"#)),
            (&["py"][..], r(r#"@\w+\.(get|post|put|patch|delete|route|api_route)\(|\b(re_)?path\(\s*r?['"]"#)),
            (&["go"][..], r(r#"\.(GET|POST|PUT|PATCH|DELETE|Get|Post|Put|Patch|Delete|Any|Handle|HandleFunc|Combo)\(\s*"/"#)),
            (&["php"][..], r(r#"Route::(get|post|put|patch|delete|any|match|resource|apiResource)\("#)),
        ]
    });
    let mut n = 0;
    let mut dosya = 0;
    let mut yigin = vec![(kok.to_path_buf(), 0)];
    while let Some((k, d)) = yigin.pop() {
        for g in std::fs::read_dir(&k).into_iter().flatten().flatten() {
            let ad = g.file_name().to_string_lossy().to_string();
            let p = g.path();
            if g.file_type().is_ok_and(|t| t.is_dir()) {
                let atla = ad.starts_with('.')
                    || matches!(
                        ad.as_str(),
                        "node_modules" | "vendor" | "target" | "bin" | "obj" | "dist" | "build" | "test" | "tests" | "spec" | "__tests__" | "e2e" | "migrations"
                    );
                if d < 12 && !atla {
                    yigin.push((p, d + 1));
                }
                continue;
            }
            let uzanti = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ad.contains(".test.") || ad.contains(".spec.") || ad.starts_with("test_") || ad.ends_with("_test.go") || ad.ends_with("Tests.cs") {
                continue;
            }
            let Some((_, re)) = desenler.iter().find(|(u, _)| u.contains(&uzanti)) else { continue };
            dosya += 1;
            if dosya > 20_000 {
                return n;
            }
            if let Some(m) = crate::guvenli::oku(&p) {
                n += re.find_iter(&m).count();
            }
        }
    }
    n
}

/// Çok az işlem: hiç işlem yok, kodda rota kaydına benzeyen ifadelerin üçte birinden az işlem var (en az 8 ifade) ya da
/// 20.000 satırdan büyük uygulamada her 2.000 satıra bir işlemden az
pub fn dusuk(satir: usize, islem: usize, rota_benzeri: usize) -> bool {
    islem == 0 || (rota_benzeri >= 8 && islem * 3 < rota_benzeri) || (satir >= 20_000 && islem * 2_000 < satir)
}

/// Haritanın özetinden kullanıcıya söylenecek kapsam satırları (boşsa kapsam tamam)
pub fn uyarilar(ozet: &Value, en: bool) -> Vec<String> {
    let mut v = vec![];
    let disi: Vec<String> = ozet["kapsam"]["disi"].as_array().into_iter().flatten().filter_map(|x| Some(format!("{} ({})", x["tur"].as_str()?, x["kanit"].as_str()?))).collect();
    if !disi.is_empty() {
        v.push(if en {
            format!("not mapped by Sezgi 0.1: {}; an AI using this map does not see these APIs", disi.join(", "))
        } else {
            format!("Sezgi 0.1 bunları haritalamaz: {}; bu haritayı kullanan yapay zekâ bu API'leri görmez", disi.join(", "))
        });
    }
    if ozet["kapsam"]["dusuk"] == true {
        let (satir, islem) = (ozet["satir"].as_u64().unwrap_or(0), ozet["endpoint_sayisi"].as_u64().unwrap_or(0));
        let rb = ozet["kapsam"]["rota_benzeri"].as_u64().unwrap_or(0);
        let olcu = if rb > islem {
            if en {
                format!("the code has about {rb} route-registration-like expressions, Sezgi found {islem} operations")
            } else {
                format!("kodda rota kaydına benzeyen yaklaşık {rb} ifade var, Sezgi {islem} işlem buldu")
            }
        } else if en {
            format!("only {islem} operations in {satir} lines of code")
        } else {
            format!("{satir} satır kodda yalnızca {islem} işlem")
        };
        v.push(if en {
            format!("LOW: {olcu}. The application probably registers its routes in a way Sezgi does not read (its own router, routes discovered by reflection, a library Sezgi does not know, or routes built at run time); an AI using this map does not see most of it → sezgi report")
        } else {
            format!("DÜŞÜK: {olcu}. Uygulama rotalarını büyük ihtimalle Sezgi'nin okumadığı bir yolla kaydediyor (kendi yönlendiricisi, yansımayla bulunan gruplar, Sezgi'nin tanımadığı bir kütüphane ya da çalışma anında kurulan rotalar); bu haritayı kullanan yapay zekâ uygulamanın çoğunu görmez → sezgi rapor")
        });
    }
    v
}
