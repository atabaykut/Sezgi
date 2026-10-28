use super::*;

fn dosya(goreli: &str, metin: &str) -> Dosya {
    ayristir_metin(&Path::new("/uygulama").join(goreli), metin).unwrap()
}

fn kapilar_(dosyalar: &[(&str, &str)]) -> (Vec<Kapi>, Value) {
    let ds: Vec<Dosya> = dosyalar.iter().map(|(g, m)| dosya(g, m)).collect();
    kapilar(Path::new("/uygulama"), &ds)
}

fn idler(k: &[Kapi]) -> Vec<String> {
    k.iter().map(|x| format!("{} {}#{}", x.id, x.kontrolcu, x.fonksiyon)).collect()
}

#[test]
fn cekim_ve_tablo_adlari() {
    for (c, t) in [
        ("photos", "photo"),
        ("categories", "category"),
        ("people", "person"),
        ("addresses", "address"),
        ("boxes", "box"),
        ("statuses", "status"),
        ("radio-stations", "radio-station"),
    ] {
        assert_eq!(tekil(c), t, "{c}");
    }
    for (t, c) in [("post", "posts"), ("category", "categories"), ("person", "people"), ("box", "boxes"), ("day", "days"), ("human", "humans")] {
        assert_eq!(cogul(t), c, "{t}");
    }
    assert_eq!(varsayilan_tablo("BlogPost"), "blog_posts");
    assert_eq!(varsayilan_tablo("Category"), "categories");
    assert_eq!(sezgi_yolu("/genres/{genre?}/songs/{song:slug}"), "/genres/:genre/songs/:song");
}

#[test]
fn kaynak_rotalari_ic_ice_sig_ve_singleton() {
    let (k, _) = kapilar_(&[(
        "routes/web.php",
        r#"<?php
Route::resource('photos.comments', CommentController::class)->shallow()->only(['index', 'show']);
Route::resource('admin/user-roles', RoleController::class)->except(['create', 'edit', 'update'])->parameters(['user-roles' => 'rol']);
Route::singleton('profile', ProfileController::class)->creatable();
Route::apiSingleton('settings', SettingsController::class);
"#,
    )]);
    let mut v = idler(&k);
    v.sort();
    assert_eq!(
        v,
        [
            "DELETE /admin/user-roles/:rol RoleController#destroy",
            "DELETE /profile ProfileController#destroy",
            "GET /admin/user-roles RoleController#index",
            "GET /admin/user-roles/:rol RoleController#show",
            "GET /comments/:comment CommentController#show",
            "GET /photos/:photo/comments CommentController#index",
            "GET /profile ProfileController#show",
            "GET /profile/create ProfileController#create",
            "GET /profile/edit ProfileController#edit",
            "GET /settings SettingsController#show",
            "PATCH /profile ProfileController#update",
            "PATCH /settings SettingsController#update",
            "POST /admin/user-roles RoleController#store",
            "POST /profile ProfileController#store",
            "PUT /profile ProfileController#update",
            "PUT /settings SettingsController#update",
        ]
    );
}

#[test]
fn degiskenler_donguler_ve_dizi_grup_nitelikleri() {
    let (k, b) = kapilar_(&[(
        "routes/api.php",
        r#"<?php
$uclar = ['ping' => PingController::class, 'durum' => DurumController::class];
$oturum = ['auth:sanctum'];
Route::group(['prefix' => 'rest', 'middleware' => $oturum, 'as' => 'rest.'], function () use ($uclar) {
    foreach ($uclar as $ad => $controller) {
        Route::match(['get', 'post'], "{$ad}{format?}", $controller)->name($ad);
    }
});
foreach (config('x.routes') as $r) {
    Route::get('dinamik/sabit', fn () => 1);
}
"#,
    )]);
    let v = idler(&k);
    for id in ["GET /api/rest/ping PingController#__invoke", "POST /api/rest/durum DurumController#__invoke", "GET /api/dinamik/sabit Api#getApiDinamikSabit"] {
        assert!(v.contains(&id.to_string()), "{id}: {v:?}");
    }
    let ping = k.iter().find(|x| x.id == "GET /api/rest/ping").unwrap();
    assert_eq!((ping.giris.as_str(), ping.giris_kaniti.as_str()), ("gerekli", "kesin"));
    // Kaynağı çözülemeyen döngüdeki kayıt koşullu
    assert!(b["kosullu_kapilar"].as_array().unwrap().iter().any(|x| x == "GET /api/dinamik/sabit"));
    assert!(!b["kosullu_kapilar"].as_array().unwrap().iter().any(|x| x == "GET /api/rest/ping"));
}

#[test]
fn kural_cevirisi() {
    let d = dosya(
        "app/Http/Requests/R.php",
        r#"<?php
class R extends \Illuminate\Foundation\Http\FormRequest {
    public function rules() {
        return [
            'ad' => 'required|string|between:2,40',
            'yas' => 'nullable|integer|min:18',
            'puan' => 'numeric|max:5',
            'etiketler' => 'required|array|min:1',
            'etiketler.*' => 'string|distinct',
            'eposta' => ['required', 'email:rfc'],
            'kod' => 'required|regex:/^[A-Z]{3}$/',
            'tur' => 'in:a,b',
            'not' => 'max:10',
        ];
    }
}
"#,
    );
    let ds = vec![d];
    let b = Baglam::yeni(Path::new("/uygulama"), &ds);
    let (dizi, satirlar, _, satir) = b.form_kurallari((0, 0), 0).unwrap();
    let (a, _, _) = b.kural_semasi(0, &dizi, &satirlar, satir);
    assert_eq!(a["ad"].kurallar, ["IsString()", "MinLength(2)", "MaxLength(40)"]);
    assert!(a["ad"].zorunlu && !a["yas"].zorunlu);
    assert_eq!(a["yas"].kurallar, ["nullable()", "IsInt()", "Min(18)"]);
    assert_eq!(a["puan"].kurallar, ["IsNumber()", "Max(5)"]);
    assert_eq!(a["etiketler"].kurallar, ["IsArray()", "ArrayMinSize(1)", "IsString({ each: true })"]);
    assert_eq!(a["eposta"].kurallar, ["IsEmail()"]);
    assert_eq!(a["kod"].kurallar, ["Matches(/^[A-Z]{3}$/)"]);
    assert_eq!(a["tur"].degerler, ["a", "b"]);
    // Tipi belirsiz alanda max metin uzunluğu mu sayı mı bilinmez: çevrilmez
    assert_eq!(a["not"].kurallar, ["max:10"]);
    // Her alan kendi satırıyla (kural kanıtları ayrışır)
    assert_ne!(a["ad"].satir, a["yas"].satir);
}
