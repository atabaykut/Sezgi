//! Dış etkinin çağrı grafiği üzerinden izlenmesi (işlem başına). Satır desenleri (dis.rs) bilinen istemci çağrılarını
//! bulur; burada işleyiciden çağrı grafiği boyunca inilir (derinlik sınırı grafiğinkiyle aynı) ve ulaşılan
//! her fonksiyonun çağrı yerlerine bakılır:
//! - **Polimorfik dağıtım:** yöntem çağrısının olası hedefleri de izlenir: taban sınıf tipli alıcıda alt sınıfların aynı
//!   adlı yöntemleri, tipi bilinmeyen alıcıda projedeki aynı adlı yöntemler (aday kümesi; tek aday da olsa güven en fazla
//!   `inferred`). Aday kümesi sınırı aşarsa hedef belirsizdir.
//! - **Kütüphane çağrısı:** hedefi projede olmayan çağrının girdiği modül (içe aktarma, tipli alan / değişken, sınıfın
//!   kütüphane tabanı) genel bir tabloyla sınıflandırılır: dış (HTTP istemcisi, e-posta, kuyruk, ödeme, bulut, bildirim),
//!   iç (standart kütüphane, ORM, çatı, loglama, JSON, tarih, doğrulama) ya da sınıflandırılamayan.
//! - **Bilinmezlik:** zincirde sınıflandırılamayan kütüphane çağrısı, dinamik çağrı (dizin erişimi, yansıma, `getattr`),
//!   belirsiz aday kümesi ya da sınıra takılıp kesilen zincir varsa işlemin dış etkisi `bilinmiyor`dur. Yazma işleminde
//!   bu onay gerektirir (fail-closed); okumada yalnızca bilgidir.
//!
//! Tablo depo adına ya da dosyasına göre değil, paket adına göredir. Bulunmayan etki "yok" demek değildir.
use crate::cagri::{Grafik, EN_DERIN};
use crate::kanit::Guven;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Dış etki izlemesinin ziyaret edeceği en fazla fonksiyon. Derinlik sınırı grafiğinkiyle aynıdır (`EN_DERIN`); fonksiyon
/// sınırı daha geniştir: polimorfik adaylar da sayılır ve kesilen zincir yazmayı onaya düşürür
const EN_FAZLA_IZLEME: usize = 2000;
/// İşlem başına tutulacak en fazla dış çağrı ve bilinmezlik nedeni
const EN_FAZLA_DIS: usize = 24;
const EN_FAZLA_NEDEN: usize = 8;

/// Modülün sınıfı
#[derive(Debug, PartialEq, Eq)]
pub enum Sinif {
    /// Uygulamanın dışına etki eder: tür (http | eposta | kuyruk | odeme | bulut | bildirim) ve eşleşen paket
    Dis(&'static str, &'static str),
    /// Uygulamanın içinde kalır (standart kütüphane, ORM, çatı, loglama, JSON, tarih, doğrulama)
    Ic,
    /// Sınıflandırılamayan dış kütüphane
    Bilinmiyor,
}

/// Dışarıya etki eden paketler (normalleştirilmiş ad öneki, tür)
const DIS_PAKETLER: &[(&str, &str)] = &[
    // HTTP istemcileri
    ("requests", "http"),
    ("httpx", "http"),
    ("aiohttp", "http"),
    ("urllib.request", "http"),
    ("urllib3", "http"),
    ("pycurl", "http"),
    ("http.client", "http"),
    ("httplib2", "http"),
    ("tornado.httpclient", "http"),
    ("axios", "http"),
    ("node-fetch", "http"),
    ("got", "http"),
    ("undici", "http"),
    ("superagent", "http"),
    ("needle", "http"),
    ("ky", "http"),
    ("cross-fetch", "http"),
    ("isomorphic-fetch", "http"),
    ("nestjs.axios", "http"),
    ("java.net.http", "http"),
    ("java.net.httpurlconnection", "http"),
    ("org.springframework.web.client", "http"),
    ("org.springframework.web.reactive.function.client", "http"),
    ("org.springframework.cloud.openfeign", "http"),
    ("okhttp3", "http"),
    ("org.apache.http", "http"),
    ("org.apache.hc", "http"),
    ("feign", "http"),
    ("retrofit2", "http"),
    ("io.ktor.client", "http"),
    ("guzzlehttp", "http"),
    ("illuminate.support.facades.http", "http"),
    ("illuminate.http.client", "http"),
    ("symfony.component.httpclient", "http"),
    ("symfony.contracts.httpclient", "http"),
    ("laravel.socialite", "http"),
    ("faraday", "http"),
    ("httparty", "http"),
    ("restclient", "http"),
    ("net.http", "http"),
    ("excon", "http"),
    ("octokit", "http"),
    ("saloon", "http"),
    ("typhoeus", "http"),
    ("github.com.go-resty", "http"),
    // E-posta
    ("smtplib", "eposta"),
    ("aiosmtplib", "eposta"),
    ("django.core.mail", "eposta"),
    ("flask_mail", "eposta"),
    ("fastapi_mail", "eposta"),
    ("emails", "eposta"),
    ("yagmail", "eposta"),
    ("sendgrid", "eposta"),
    ("postmarker", "eposta"),
    ("nodemailer", "eposta"),
    ("resend", "eposta"),
    ("postmark", "eposta"),
    ("mailgun", "eposta"),
    ("mailgun.js", "eposta"),
    ("javax.mail", "eposta"),
    ("jakarta.mail", "eposta"),
    ("org.springframework.mail", "eposta"),
    ("illuminate.support.facades.mail", "eposta"),
    ("illuminate.mail", "eposta"),
    ("symfony.component.mailer", "eposta"),
    ("phpmailer", "eposta"),
    ("net.smtp", "eposta"),
    ("mail", "eposta"),
    // Kuyruk ve olay yayını
    ("celery", "kuyruk"),
    ("kombu", "kuyruk"),
    ("pika", "kuyruk"),
    ("aio_pika", "kuyruk"),
    ("kafka", "kuyruk"),
    ("confluent_kafka", "kuyruk"),
    ("aiokafka", "kuyruk"),
    ("kafkajs", "kuyruk"),
    ("amqplib", "kuyruk"),
    ("amqp-connection-manager", "kuyruk"),
    ("bull", "kuyruk"),
    ("bullmq", "kuyruk"),
    ("bee-queue", "kuyruk"),
    ("rq", "kuyruk"),
    ("dramatiq", "kuyruk"),
    ("nats", "kuyruk"),
    ("nestjs.microservices", "kuyruk"),
    ("org.springframework.kafka", "kuyruk"),
    ("org.springframework.amqp", "kuyruk"),
    ("org.springframework.jms", "kuyruk"),
    ("org.apache.kafka", "kuyruk"),
    ("com.rabbitmq", "kuyruk"),
    ("google.cloud.pubsub", "kuyruk"),
    ("google-cloud.pubsub", "kuyruk"),
    ("github.com.segmentio.kafka-go", "kuyruk"),
    ("github.com.rabbitmq", "kuyruk"),
    ("github.com.streadway.amqp", "kuyruk"),
    ("sidekiq", "kuyruk"),
    ("resque", "kuyruk"),
    ("bunny", "kuyruk"),
    // Ödeme
    ("stripe", "odeme"),
    ("com.stripe", "odeme"),
    ("braintree", "odeme"),
    ("com.braintreegateway", "odeme"),
    ("paypalrestsdk", "odeme"),
    ("paypal", "odeme"),
    ("plaid", "odeme"),
    ("adyen", "odeme"),
    ("squareup", "odeme"),
    ("mollie", "odeme"),
    ("razorpay", "odeme"),
    ("iyzipay", "odeme"),
    // Bulut
    ("boto3", "bulut"),
    ("botocore", "bulut"),
    ("aiobotocore", "bulut"),
    ("aws-sdk", "bulut"),
    ("aws", "bulut"),
    ("com.amazonaws", "bulut"),
    ("software.amazon.awssdk", "bulut"),
    ("github.com.aws", "bulut"),
    ("google.cloud", "bulut"),
    ("google-cloud", "bulut"),
    ("googleapiclient", "bulut"),
    ("azure", "bulut"),
    ("minio", "bulut"),
    ("cloudinary", "bulut"),
    ("dropbox", "bulut"),
    // Bildirim
    ("twilio", "bildirim"),
    ("pywebpush", "bildirim"),
    ("web-push", "bildirim"),
    ("apns2", "bildirim"),
    ("pyfcm", "bildirim"),
    ("firebase_admin", "bildirim"),
    ("firebase-admin", "bildirim"),
    ("slack_sdk", "bildirim"),
    ("slack.web-api", "bildirim"),
    ("discord.js", "bildirim"),
    ("telegram", "bildirim"),
    ("onesignal", "bildirim"),
    ("illuminate.support.facades.notification", "bildirim"),
    ("illuminate.notifications", "bildirim"),
    ("pusher", "bildirim"),
    ("apprise", "bildirim"),
    ("vonage", "bildirim"),
    ("nexmo", "bildirim"),
];

/// Uygulamanın içinde kalan paketler: standart kütüphane, çatı, ORM / veritabanı sürücüsü, önbellek, loglama ve
/// gözlem, JSON / serileştirme, tarih, doğrulama, şifreleme, şablon ve metin işleme
const IC_PAKETLER: &[&str] = &[
    // Python standart kütüphanesi
    "__future__",
    "json",
    "datetime",
    "time",
    "re",
    "typing",
    "typing_extensions",
    "logging",
    "uuid",
    "hashlib",
    "hmac",
    "secrets",
    "base64",
    "math",
    "decimal",
    "fractions",
    "statistics",
    "random",
    "string",
    "textwrap",
    "collections",
    "itertools",
    "functools",
    "operator",
    "dataclasses",
    "enum",
    "abc",
    "copy",
    "contextlib",
    "io",
    "os.path",
    "pathlib",
    "shutil",
    "tempfile",
    "zoneinfo",
    "calendar",
    "binascii",
    "struct",
    "html",
    "urllib.parse",
    "urllib",
    "ipaddress",
    "email",
    "unicodedata",
    "locale",
    "gettext",
    "warnings",
    "traceback",
    "inspect",
    "types",
    "weakref",
    "heapq",
    "bisect",
    "array",
    "pprint",
    "csv",
    "zlib",
    "gzip",
    "zipfile",
    "tarfile",
    "mimetypes",
    "threading",
    "asyncio",
    "concurrent",
    "queue",
    "sys",
    "platform",
    "contextvars",
    "codecs",
    "difflib",
    "fnmatch",
    "glob",
    "numbers",
    "pickle",
    "shlex",
    "sqlite3",
    "http",
    "xml",
    "hashids",
    "graphlib",
    // Python çatıları, ORM ve yardımcılar
    "django",
    "rest_framework",
    "fastapi",
    "starlette",
    "flask",
    "werkzeug",
    "jinja2",
    "pydantic",
    "pydantic_settings",
    "sqlalchemy",
    "sqlmodel",
    "alembic",
    "marshmallow",
    "jwt",
    "jose",
    "passlib",
    "bcrypt",
    "argon2",
    "cryptography",
    "itsdangerous",
    "dateutil",
    "pytz",
    "arrow",
    "pendulum",
    "orjson",
    "ujson",
    "simplejson",
    "yaml",
    "toml",
    "tomllib",
    "markdown",
    "bleach",
    "nh3",
    "humanize",
    "babel",
    "pil",
    "numpy",
    "pandas",
    "structlog",
    "loguru",
    "sentry_sdk",
    "redis",
    "psycopg",
    "psycopg2",
    "pymysql",
    "asyncpg",
    "motor",
    "pymongo",
    "mongoengine",
    "peewee",
    "tortoise",
    "click",
    "typer",
    "rich",
    "attrs",
    "attr",
    "cattrs",
    "msgspec",
    "cachetools",
    "tenacity",
    "slugify",
    "unidecode",
    "phonenumbers",
    "email_validator",
    "validators",
    "cronsim",
    "croniter",
    "oncalendar",
    "fido2",
    "pyotp",
    "qrcode",
    "segno",
    "statsd",
    "prometheus_client",
    "opentelemetry",
    "whitenoise",
    "lxml",
    "bs4",
    "defusedxml",
    "markupsafe",
    "polib",
    "emoji",
    "multipart",
    "python_multipart",
    "anyio",
    "httpcore",
    // JavaScript / TypeScript (Node yerleşikleri dahil)
    "nestjs",
    "express",
    "fastify",
    "koa",
    "hono",
    "next",
    "lodash",
    "lodash-es",
    "underscore",
    "ramda",
    "date-fns",
    "dayjs",
    "moment",
    "luxon",
    "zod",
    "joi",
    "yup",
    "class-validator",
    "class-transformer",
    "typeorm",
    "prisma",
    "sequelize",
    "mongoose",
    "kysely",
    "knex",
    "pg",
    "mysql2",
    "mysql",
    "sqlite3",
    "better-sqlite3",
    "ioredis",
    "uuid",
    "nanoid",
    "cuid",
    "bcryptjs",
    "jsonwebtoken",
    "crypto",
    "path",
    "fs",
    "util",
    "events",
    "stream",
    "buffer",
    "url",
    "querystring",
    "os",
    "rxjs",
    "reflect-metadata",
    "multer",
    "passport",
    "passport-jwt",
    "passport-local",
    "cookie",
    "cookie-parser",
    "express-session",
    "winston",
    "pino",
    "debug",
    "chalk",
    "sanitize-html",
    "validator",
    "marked",
    "handlebars",
    "ejs",
    "pug",
    "express-async-handler",
    "http-errors",
    "mime",
    "mime-types",
    "sharp",
    "archiver",
    "csv-parse",
    "dotenv",
    "config",
    "helmet",
    "cors",
    "compression",
    "morgan",
    "body-parser",
    "express-validator",
    "socket.io",
    "assert",
    "async_hooks",
    "timers",
    "perf_hooks",
    "worker_threads",
    "readline",
    "string_decoder",
    "zlib",
    "colors",
    "nestjs-zod",
    "transformation-matrix",
    "react",
    "react-dom",
    "react-email",
    "sanitize-filename",
    "mnemonist",
    "nestjs-cls",
    "kysely-postgres-js",
    "postgres",
    "luxon",
    "exiftool-vendored",
    "fluent-ffmpeg",
    "geo-tz",
    "i18n-iso-countries",
    "picomatch",
    "semver",
    "handlebars",
    "ua-parser-js",
    "js-yaml",
    "async-lock",
    "openid-client",
    // Java / Kotlin
    "java",
    "javax",
    "jakarta",
    "org.springframework",
    "lombok",
    "com.fasterxml",
    "org.slf4j",
    "org.apache.commons",
    "org.apache.logging",
    "com.google.common",
    "com.google.gson",
    "io.jsonwebtoken",
    "org.hibernate",
    "kotlin",
    "kotlinx",
    "io.ktor",
    "org.jetbrains.exposed",
    "org.mapstruct",
    "org.modelmapper",
    "io.swagger",
    "org.joda",
    "io.micrometer",
    "reactor",
    "io.reactivex",
    "com.auth0.jwt",
    "org.json",
    "com.zaxxer",
    "org.flywaydb",
    "liquibase",
    "com.querydsl",
    "org.jooq",
    "org.mindrot",
    "org.owasp",
    "org.springdoc",
    "graphql",
    "com.netflix.graphql",
    // Go standart kütüphanesi dışındaki yaygın paketler (standart kütüphane ayrıca tanınır)
    "gorm.io",
    "github.com.gin-gonic",
    "github.com.labstack",
    "github.com.gofiber",
    "github.com.go-chi",
    "github.com.gorilla",
    "github.com.google.uuid",
    "github.com.golang-jwt",
    "github.com.dgrijalva",
    "golang.org.x.crypto",
    "golang.org.x.text",
    "go.uber.org",
    "github.com.sirupsen",
    "github.com.spf13",
    "github.com.go-playground",
    "github.com.jmoiron",
    "github.com.lib.pq",
    "github.com.jackc",
    "github.com.go-sql-driver",
    "github.com.mattn",
    "github.com.redis",
    "github.com.go-redis",
    "github.com.casbin",
    "github.com.swaggo",
    "github.com.joho",
    "go.mongodb.org",
    "github.com.pkg.errors",
    "github.com.shopspring",
    "github.com.mojocn",
    "gopkg.in.go-playground",
    "gopkg.in.yaml",
    // PHP
    "illuminate",
    "symfony",
    "carbon",
    "ramsey",
    "league",
    "laravel",
    "intervention",
    "spatie",
    "barryvdh",
    "psr",
    "monolog",
    "doctrine",
    "firebase.jwt",
    "predis",
    "nesbot",
    "webpatser",
    "maatwebsite",
    "enshrined",
    "iplib",
    "webmozart",
    "htmlpurifier",
    "ezyang",
    "robthree",
    "dompdf",
    "phpseclib",
    "phpseclib3",
    "jaybizzle",
    // Hata izleme ve başarım ölçümü (uygulamanın verisini dışarıda değiştirmez)
    "sentry",
    "bugsnag",
    "honeybadger",
    "rollbar",
    "airbrake",
    "newrelic",
    "datadog",
    "ddtrace",
    "skylight",
    "appsignal",
    // Ruby (sabitler: çekirdek, Rails ve yaygın taşlar)
    "time",
    "date",
    "datetime",
    "json",
    "yaml",
    "rails",
    "activerecord",
    "activesupport",
    "activemodel",
    "actioncontroller",
    "actionview",
    "actiondispatch",
    "activejob",
    "activestorage",
    "actioncable",
    "i18n",
    "integer",
    "float",
    "string",
    "array",
    "hash",
    "set",
    "struct",
    "regexp",
    "math",
    "bigdecimal",
    "securerandom",
    "digest",
    "openssl",
    "base64",
    "uri",
    "cgi",
    "erb",
    "logger",
    "pathname",
    "file",
    "fileutils",
    "dir",
    "io",
    "stringio",
    "kernel",
    "object",
    "comparable",
    "enumerable",
    "rational",
    "rack",
    "concurrent",
    "nokogiri",
    "redcarpet",
    "commonmarker",
    "money",
    "pagy",
    "kaminari",
    "devise",
    "warden",
    "pundit",
    "cancan",
    "current",
    "zlib",
    "timeout",
    "benchmark",
    "csv",
    "ostruct",
    "openstruct",
    "rouge",
    "loofah",
    "addressable",
    "turbo",
    "turbostreams",
    "rails.logger",
    "rails.cache",
    "jwt",
    "bcrypt",
    "rotp",
    "rqrcode",
    "mime",
    "marcel",
    "vips",
    "minimagick",
    "range",
    "symbol",
    "nilclass",
    "trueclass",
    "falseclass",
    "numeric",
    "proc",
    "method",
    "class",
    "module",
    "exception",
    "standarderror",
    "argumenterror",
    "runtimeerror",
    "notimplementederror",
    "process",
    "signal",
    "gc",
    "objectspace",
    "thread",
    "mutex",
    "queue",
];

/// Go standart kütüphanesinde dışarıya giden yollar: (paket, yöntem; boşsa paketin tamamı, tür ya da "?" bilinmiyor)
const GO_DIS: &[(&str, &str, &str)] = &[
    ("net.http", "Get", "http"),
    ("net.http", "Post", "http"),
    ("net.http", "PostForm", "http"),
    ("net.http", "Head", "http"),
    ("net.http", "NewRequest", "http"),
    ("net.http", "NewRequestWithContext", "http"),
    ("net.smtp", "", "eposta"),
    ("os.exec", "", "?"),
    ("net", "Dial", "?"),
    ("net", "DialTimeout", "?"),
    ("net", "DialTCP", "?"),
    ("net", "DialUDP", "?"),
];

/// Paket adını karşılaştırma biçimine getirir: küçük harf, ayraçlar nokta, baştaki `@`, `node:` ve `rb:` atılır
fn normallestir(modul: &str) -> (String, bool) {
    let m = modul.trim().trim_start_matches(['\\', '@']);
    let (ruby, m) = match m.strip_prefix("rb:") {
        Some(r) => (true, r),
        None => (false, m.strip_prefix("php:").unwrap_or(m)),
    };
    let m = m.strip_prefix("node:").unwrap_or(m);
    let mut n = m.to_ascii_lowercase().replace("::", ".").replace(['\\', '/', ':'], ".");
    while n.contains("..") {
        n = n.replace("..", ".");
    }
    (n.trim_matches('.').to_string(), ruby)
}

fn onek(m: &str, p: &str) -> bool {
    m == p || (m.starts_with(p) && m.as_bytes().get(p.len()) == Some(&b'.'))
}

/// Çağrının girdiği modülü (ve çağrılan adı) sınıflandırır
pub fn sinifla(modul: &str, ad: &str) -> Sinif {
    let (m, ruby) = normallestir(modul);
    if m.is_empty() {
        return Sinif::Ic;
    }
    // Hata nesnesi kurmak ya da fırlatmak uygulamanın içindedir
    let son = modul.rsplit(['.', '\\', ':', '/']).next().unwrap_or("");
    if [son, ad].iter().any(|a| a.ends_with("Exception") || a.ends_with("Error") || *a == "Throwable") {
        return Sinif::Ic;
    }
    // PHP çekirdek sınıfı (ad alanı yok: `ZipArchive`, `DOMDocument`)
    if modul.strip_prefix("php:").is_some_and(|r| !r.trim_start_matches('\\').contains('\\')) {
        return Sinif::Ic;
    }
    // Go standart kütüphanesi: ilk parçasında nokta olmayan yol (`net/http`, `encoding/json`)
    let go_std =
        !ruby && !modul.contains(['.', '\\', '@', ':']) && modul.contains('/') || matches!(modul, "fmt" | "errors" | "strings" | "strconv" | "context" | "sort" | "bytes" | "net");
    if go_std || matches!(m.as_str(), "net" | "os.exec") {
        for (p, y, tur) in GO_DIS {
            if onek(&m, p) && (y.is_empty() || *y == ad) {
                return if *tur == "?" { Sinif::Bilinmiyor } else { Sinif::Dis(tur, p) };
            }
        }
        if go_std {
            return Sinif::Ic;
        }
    }
    // Node yerleşik http / https: sunucu kurmak iç, istek göndermek dış
    if matches!(m.as_str(), "http" | "https") && !ruby && matches!(ad, "request" | "get") && !modul.contains('.') {
        return Sinif::Dis("http", "node:http");
    }
    // Süreç başlatma: dışarıya etkisi koddan sınıflandırılamaz
    if matches!(m.as_str(), "subprocess" | "child_process") || (m == "os" && (ad == "system" || ad.starts_with("popen") || ad.starts_with("exec") || ad.starts_with("spawn"))) {
        return Sinif::Bilinmiyor;
    }
    if let Some((p, tur)) = DIS_PAKETLER.iter().filter(|(p, _)| onek(&m, p)).max_by_key(|(p, _)| p.len()) {
        return Sinif::Dis(tur, p);
    }
    // Ruby'de içe aktarma yoktur: sabitin projeye mi bir taşa mı ait olduğu koddan bilinmez; yalnızca tablodaki dış
    // paketler sayılır
    if ruby || IC_PAKETLER.iter().any(|p| onek(&m, p)) {
        return Sinif::Ic;
    }
    Sinif::Bilinmiyor
}

/// Modülün gösterilen adı (dil öneki atılır)
fn gorunen_modul(m: &str) -> &str {
    m.trim_start_matches("rb:").trim_start_matches("php:")
}

/// Çağrı adı okuma mı (HTTP GET / HEAD, SDK'da get / list / describe / retrieve ...)
fn okuma_mi(tur: &str, ad: &str) -> bool {
    let a = ad.to_ascii_lowercase();
    match tur {
        "http" => matches!(a.as_str(), "get" | "head" | "options" | "getforobject" | "getforentity"),
        "bulut" | "odeme" => ["get", "list", "describe", "retrieve", "head", "download", "search", "query", "scan", "read", "fetch", "find"].iter().any(|p| a.starts_with(p)),
        _ => false,
    }
}

/// Bir işlemin çağrı grafiği üzerinden dış etki sonucu
#[derive(Default, Clone)]
pub struct IslemDis {
    /// Yalnızca polimorfik aday üzerinden ulaşılan fonksiyonlar: (dosya, ilk satır, son satır, derinlik, yol); satır
    /// desenleriyle bulunan dış çağrılar bunlarda ayrıca aranır (çağrı kenarıyla ulaşılanlara dis.rs zaten bakar)
    pub adaydan: Vec<(String, usize, usize, usize, Vec<String>)>,
    /// Sınıflandırılan dış çağrılar (davranış özetinin `dis` biçiminde)
    pub disler: Vec<Value>,
    /// Bilinmezlik nedenleri: {tur: kutuphane | dinamik | belirsiz | kesildi, ifade, kanit}
    pub bilinmeyen: Vec<Value>,
    pub bilinmeyen_sayisi: usize,
}

/// Bir düğüm: fonksiyon, derinlik, yolun en zayıf güveni, geldiği düğüm
struct Dugum {
    fonk: usize,
    derinlik: usize,
    guven: Guven,
    once: Option<usize>,
    /// Yolda polimorfik aday halkası var mı
    aday: bool,
}

/// İşleyiciden genişlik öncelikli iniş: çağrı kenarları ve polimorfik adaylar (aday halkası en fazla inferred)
pub fn izle(g: &Grafik, isleyici: usize) -> IslemDis {
    let mut d: Vec<Dugum> = vec![Dugum { fonk: isleyici, derinlik: 0, guven: Guven::Certain, once: None, aday: false }];
    let mut gorulen: HashSet<usize> = HashSet::from([isleyici]);
    let mut kuyruk: VecDeque<usize> = VecDeque::from([0]);
    let mut kesildi = false;
    while let Some(u) = kuyruk.pop_front() {
        let (f, dr, gv, ad0) = (d[u].fonk, d[u].derinlik, d[u].guven, d[u].aday);
        if g.denetim[f] && dr > 0 {
            continue;
        }
        // Yalnızca projede tek olan adla kurulan kenar başka dosyaya izlenmez: çağrı nesnenin sınıfında / dosyasında
        // çözülemediyse aynı adlı uzak fonksiyon onun hedefi sayılmaz (yanlış zincir, yanlış dış etki)
        let kenarlar = g.giden[f]
            .iter()
            .filter(|&&e| g.kenarlar[e].cozum != "ad" || g.fonksiyonlar[g.kenarlar[e].hedef].dosya == g.fonksiyonlar[f].dosya)
            .map(|&e| (g.kenarlar[e].hedef, g.kenarlar[e].guven, false));
        let adaylar = g.cagri_yerleri[f].iter().flat_map(|y| y.adaylar.iter().map(|&a| (a, Guven::Inferred, true)));
        for (h, kg, aday) in kenarlar.chain(adaylar).collect::<Vec<_>>() {
            if gorulen.contains(&h) {
                continue;
            }
            if dr + 1 > EN_DERIN || d.len() >= EN_FAZLA_IZLEME {
                kesildi = true;
                continue;
            }
            gorulen.insert(h);
            kuyruk.push_back(d.len());
            d.push(Dugum { fonk: h, derinlik: dr + 1, guven: gv.min(kg), once: Some(u), aday: ad0 || aday });
        }
    }
    let yol = |mut i: usize| -> Vec<String> {
        let mut v = vec![g.fonksiyonlar[d[i].fonk].gorunen()];
        while let Some(o) = d[i].once {
            v.push(g.fonksiyonlar[d[o].fonk].gorunen());
            i = o;
        }
        v.reverse();
        v
    };
    let mut s = IslemDis::default();
    let neden = |s: &mut IslemDis, v: Value| {
        s.bilinmeyen_sayisi += 1;
        if s.bilinmeyen.len() < EN_FAZLA_NEDEN && !s.bilinmeyen.iter().any(|x| x["kanit"] == v["kanit"] && x["tur"] == v["tur"]) {
            s.bilinmeyen.push(v);
        }
    };
    // Aynı fonksiyonda aynı pakete giden çağrılar tek dış etki sayılır
    let mut eklenen: HashSet<(usize, &'static str)> = HashSet::new();
    for (i, u) in d.iter().enumerate() {
        let fk = &g.fonksiyonlar[u.fonk];
        if u.aday {
            s.adaydan.push((fk.dosya.clone(), fk.satir, fk.son, u.derinlik, yol(i)));
        }
        if g.denetim[u.fonk] && u.derinlik > 0 {
            continue;
        }
        for y in &g.cagri_yerleri[u.fonk] {
            let kanit = format!("{}:{}", fk.dosya, y.satir);
            // Parametre olarak gelen fonksiyonun çağrısı (geri çağırma, `next()`): hedefi çağıranın verdiği koddur, yansıma
            // ya da dizinle seçim değildir; `.bind(...)` çağırmaz, bağlar
            let geri_cagirma = y.ifade.ends_with(".bind")
                || (!y.ifade.contains(['.', '[', '<', ':'])
                    && !matches!(
                        y.ifade.as_str(),
                        "send" | "public_send" | "__send__" | "instance_eval" | "instance_exec" | "method" | "eval" | "exec" | "call_user_func" | "call_user_func_array"
                    ));
            if y.dinamik && !geri_cagirma {
                neden(&mut s, json!({ "tur": "dinamik", "ifade": y.ifade, "kanit": kanit, "zincir": yol(i) }));
                continue;
            }
            if y.dinamik {
                continue;
            }
            if y.belirsiz {
                neden(&mut s, json!({ "tur": "belirsiz", "ifade": y.ifade, "kanit": kanit, "zincir": yol(i) }));
                continue;
            }
            let (Some(m), None) = (&y.modul, y.hedef) else { continue };
            let ad = y.ifade.rsplit(['.', ':', '\\', '>']).next().unwrap_or(&y.ifade);
            match sinifla(m, ad) {
                Sinif::Ic => {}
                Sinif::Bilinmiyor => neden(&mut s, json!({ "tur": "kutuphane", "ifade": y.ifade, "modul": gorunen_modul(m), "kanit": kanit, "zincir": yol(i) })),
                Sinif::Dis(tur, paket) => {
                    if s.disler.len() >= EN_FAZLA_DIS || !eklenen.insert((u.fonk, paket)) {
                        continue;
                    }
                    let yontem = (tur == "http").then(|| ad.to_ascii_uppercase()).filter(|a| matches!(a.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"));
                    let mut v = json!({
                        "tur": tur,
                        "istemci": gorunen_modul(m),
                        "yontem": yontem,
                        "hedef": paket,
                        "hedef_turu": "sdk",
                        "dugum": format!("external:{paket}"),
                        "yazma": !okuma_mi(tur, ad),
                        "zaman": "senkron",
                        "kanit": kanit,
                        "fonksiyon": fk.gorunen(),
                        "derinlik": u.derinlik,
                        // Paketin dışarıya gittiği bir sınıflandırmadır; zincir de en çok yolun en zayıf halkası kadar güvenilir
                        "guven": u.guven.min(Guven::Inferred).ad(),
                        "kaynak": "cagri_grafigi",
                    });
                    let z = yol(i);
                    if z.len() > 1 {
                        v["zincir"] = json!(z);
                    }
                    s.disler.push(v);
                }
            }
        }
    }
    if kesildi {
        neden(&mut s, json!({ "tur": "kesildi", "ifade": format!("{EN_DERIN} / {EN_FAZLA_IZLEME}"), "kanit": Value::Null }));
    }
    s
}

/// Bütün işlemler için izleme: "işlem-kimliği dosya:satır" -> sonuç
pub fn hesapla(g: &Grafik, kapilar: &[crate::adaptor::Kapi]) -> HashMap<String, IslemDis> {
    // Aynı işleyiciyi paylaşan işlemler (GET + POST aynı görünüm) aynı izlemeyi kullanır
    let mut izlenen: HashMap<usize, IslemDis> = HashMap::new();
    let mut sonuc: HashMap<String, IslemDis> = HashMap::new();
    for k in kapilar {
        let Some(h) = g.isleyici_bul(&k.dosya, &k.fonksiyon, k.satir) else { continue };
        let s = izlenen.entry(h).or_insert_with(|| izle(g, h)).clone();
        sonuc.insert(format!("{} {}:{}", k.id, k.dosya, k.satir), s);
    }
    sonuc
}

/// Bilinmezlik nedeninin kısa metni (haritaya Türkçe yazılır; İngilizcesi `dis::gerekce_metni` kalıplarıyla)
fn neden_metni(n: &Value) -> String {
    let kanit = n["kanit"].as_str().map(|k| format!(" ({k})")).unwrap_or_default();
    match n["tur"].as_str().unwrap_or("") {
        "kutuphane" => format!("sınıflandırılamayan kütüphane çağrısı: {}{kanit}", n["ifade"].as_str().unwrap_or("")),
        "dinamik" => format!("hedefi koddan çıkmayan dinamik çağrı: {}{kanit}", n["ifade"].as_str().unwrap_or("")),
        "belirsiz" => format!("aday sayısı sınırı aşan yöntem çağrısı: {}{kanit}", n["ifade"].as_str().unwrap_or("")),
        _ => "çağrı zinciri izleme sınırında kesildi".to_string(),
    }
}

/// Grafik izlemesini davranış özetine işler: dış çağrılar (`dis`) eklenir, işlem başına `dis_etki` (var | bilinmiyor |
/// bulunmadi) ve bilinmezlik nedenleri yazılır. Yazma işleminde dış etki ya da bilinmezlik onay gerekçesi olur
/// (`onaylar`, haritada `etki_onayi`). Uygulama düzeyi envantere (`ozet`) sistemler ve sayılar eklenir. Dönen: dış etkisi
/// bilinmeyen yazma işlemlerinin anahtarları ("işlem-kimliği dosya:satır")
pub fn isle(
    sonuc: &HashMap<String, IslemDis>,
    tarama: &crate::dis::Tarama,
    kapilar: &[crate::adaptor::Kapi],
    dna: &mut Value,
    onaylar: &mut BTreeMap<String, String>,
    ozet: &mut Value,
) -> HashSet<String> {
    let mut bilinmeyen_islemler = HashSet::new();
    let (mut bilinmiyor_yazma, mut bilinmiyor_okuma, mut grafikten) = (0usize, 0usize, 0usize);
    let mut sistemler: BTreeMap<String, Value> = BTreeMap::new();
    for k in kapilar {
        let anahtar = format!("{} {}:{}", k.id, k.dosya, k.satir);
        let yer = format!("{}:{}", k.dosya, k.satir);
        let okuma = matches!(k.yontem.as_str(), "GET" | "HEAD" | "OPTIONS");
        let Some(s) = sonuc.get(&anahtar) else { continue };
        if dna[yer.as_str()].is_null() {
            dna[yer.as_str()] = json!({ "hatalar": [], "sahiplik_denetimi": [], "dallar": [], "yumusak_silme": null, "kapsam": "acik" });
        }
        let v = &mut dna[yer.as_str()];
        let mut disler = v["dis"].as_array().cloned().unwrap_or_default();
        let desenden = s
            .adaydan
            .iter()
            .flat_map(|(dosya, bas, son, dr, yol)| crate::dis::araliktaki_disler(tarama, dosya, *bas, *son, yol.last().map(String::as_str).unwrap_or(""), yol, *dr));
        for x in s.disler.iter().cloned().chain(desenden) {
            if disler.len() < EN_FAZLA_DIS && !disler.iter().any(|z| z["kanit"] == x["kanit"]) {
                grafikten += 1;
                let dg = x["dugum"].as_str().unwrap_or("").to_string();
                let e = sistemler
                    .entry(dg.clone())
                    .or_insert_with(|| json!({ "dugum": dg, "tur": x["tur"], "hedef": x["hedef"], "hedef_turu": "sdk", "guven": "inferred", "cagri": 0, "islemler": [] }));
                e["cagri"] = json!(e["cagri"].as_u64().unwrap_or(0) + 1);
                let mut l: Vec<Value> = e["islemler"].as_array().cloned().unwrap_or_default();
                if !l.iter().any(|i| i == k.id.as_str()) {
                    l.push(json!(k.id));
                }
                e["islemler"] = json!(l);
                disler.push(x);
            }
        }
        if !disler.is_empty() {
            v["dis"] = json!(disler);
        }
        let durum = if !disler.is_empty() {
            "var"
        } else if s.bilinmeyen_sayisi > 0 {
            "bilinmiyor"
        } else {
            "bulunmadi"
        };
        v["dis_etki"] = json!(durum);
        if s.bilinmeyen_sayisi > 0 {
            v["dis_etki_bilinmeyen"] = json!(s.bilinmeyen);
            v["dis_etki_bilinmeyen_sayisi"] = json!(s.bilinmeyen_sayisi);
        }
        if durum == "bilinmiyor" {
            if okuma {
                bilinmiyor_okuma += 1;
            } else {
                bilinmeyen_islemler.insert(anahtar.clone());
                bilinmiyor_yazma += 1;
            }
        }
        if okuma || onaylar.contains_key(&anahtar) {
            continue;
        }
        let yazan: Vec<String> = disler.iter().filter(|d| d["yazma"] == true).map(crate::dis::etki_adi).collect();
        let gerekce = if !yazan.is_empty() {
            let mut d = yazan;
            d.dedup();
            Some(format!("işlem uygulamanın dışına etki ediyor ({}): dış sistemdeki etki geri alınamaz", d.into_iter().take(3).collect::<Vec<_>>().join(", ")))
        } else if durum == "bilinmiyor" {
            let n = s.bilinmeyen.first().map(neden_metni).unwrap_or_default();
            Some(format!("işlemin dış etkisi bilinmiyor ({n}): dışarıya gidip gitmediği kanıtlanmadı"))
        } else {
            None
        };
        if let Some(g) = gerekce {
            onaylar.insert(anahtar, g.clone());
            if v["onay_gerekcesi"].is_null() {
                v["onay_gerekcesi"] = json!(g);
            }
        }
    }
    if ozet.is_object() {
        let mut liste = ozet["sistemler"].as_array().cloned().unwrap_or_default();
        for (dg, s) in sistemler {
            match liste.iter_mut().find(|x| x["dugum"] == dg.as_str()) {
                Some(x) => {
                    let mut l: Vec<Value> = x["islemler"].as_array().cloned().unwrap_or_default();
                    for i in s["islemler"].as_array().into_iter().flatten() {
                        if !l.contains(i) {
                            l.push(i.clone());
                        }
                    }
                    x["islemler"] = json!(l);
                }
                None => liste.push(s),
            }
        }
        ozet["sistemler"] = json!(liste);
        ozet["istatistik"]["grafikten_dis"] = json!(grafikten);
        ozet["istatistik"]["dis_etki_bilinmiyor_yazma"] = json!(bilinmiyor_yazma);
        ozet["istatistik"]["dis_etki_bilinmiyor_okuma"] = json!(bilinmiyor_okuma);
    }
    bilinmeyen_islemler
}

/// Haritanın kapılarına dış etkisi bilinmeyen yazma işlemlerini `dis_etki: "bilinmiyor"` olarak yazar (canlı modun onay kuralı okur)
pub fn kapilara_yaz(endpointler: &mut Value, bilinmeyen: &HashSet<String>) {
    for e in endpointler.as_array_mut().into_iter().flatten() {
        let anahtar = format!("{} {}:{}", e["id"].as_str().unwrap_or(""), e["dosya"].as_str().unwrap_or(""), e["satir"]);
        if bilinmeyen.contains(&anahtar) {
            e["dis_etki"] = json!("bilinmiyor");
        }
    }
}

/// `tani` ve Ayna için dış etki durumu satırları: bilinmiyorsa nedenleri (zinciriyle)
pub fn tani_satirlari(v: &Value, en: bool) -> Vec<String> {
    let d = |tr: &str, e: &str| if en { e.to_string() } else { tr.to_string() };
    let mut s = vec![];
    match v["dis_etki"].as_str() {
        Some("bilinmiyor") => {
            let n = v["dis_etki_bilinmeyen_sayisi"].as_u64().unwrap_or(0);
            s.push(format!(
                "  {}: {} ({n} {}) [{}]",
                d("dış etki", "external effect"),
                d("bilinmiyor", "unknown"),
                d("sınıflandırılamayan çağrı", crate::dil::cogul(n, "unclassified call", "unclassified calls")),
                d("Bilinmiyor", "Unknown")
            ));
            for x in v["dis_etki_bilinmeyen"].as_array().into_iter().flatten().take(4) {
                let zincir = x["zincir"]
                    .as_array()
                    .map(|z| z.iter().filter_map(|a| a.as_str()).collect::<Vec<_>>().join(" → "))
                    .filter(|z| !z.is_empty())
                    .map(|z| format!(" ← {z}"))
                    .unwrap_or_default();
                s.push(format!("    - {}{zincir}", crate::dis::gerekce_cevir(&neden_metni(x), en)));
            }
        }
        Some("bulunmadi") => s.push(format!(
            "  {}: {} [{}]",
            d("dış etki", "external effect"),
            d("zincirde bulunmadı (bütün çağrılar sınıflandırıldı)", "none found in the chain (every call classified)"),
            d("Yorum", "Inferred")
        )),
        _ => {}
    }
    s
}
