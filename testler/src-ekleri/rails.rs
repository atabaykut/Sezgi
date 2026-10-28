use super::*;

fn dosya(goreli: &str, metin: &str) -> Dosya {
    ayristir_metin(goreli, metin).unwrap()
}

#[test]
fn cekim_kurallari() {
    for (c, t) in [("posts", "post"), ("categories", "category"), ("statuses", "status"), ("people", "person"), ("addresses", "address"), ("boxes", "box"), ("line_items", "line_item"), ("news", "news"), ("archives", "archive"), ("wolves", "wolf")] {
        assert_eq!(tekil(c), t, "{c}");
    }
    for (t, c) in [("post", "posts"), ("category", "categories"), ("status", "statuses"), ("person", "people"), ("box", "boxes"), ("line_item", "line_items"), ("arsiv_kaydi", "arsiv_kaydis"), ("profile", "profiles")] {
        assert_eq!(cogul(t), c, "{t}");
    }
    assert_eq!(camelize("api/v1/blog_posts"), "Api::V1::BlogPosts");
    assert_eq!(underscore("ArsivKaydi"), "arsiv_kaydi");
    assert_eq!(underscore("APIKey"), "api_key");
    assert_eq!(snake_camel("mark_read"), "markRead");
}

fn rotalar(metin: &str, ek: &[(&str, &str)]) -> Vec<String> {
    let mut ds = vec![dosya("config/routes.rb", metin)];
    ds.extend(ek.iter().map(|(g, m)| dosya(g, m)));
    let p = Proje::yeni(Path::new("olmayan-klasor"), &ds);
    rotalari_oku(&p).rotalar.iter().map(|r| format!("{} {} {}#{}", r.yontem, r.yol, r.kontrolcu, r.eylem)).collect()
}

#[test]
fn rota_dsl_bicimleri() {
    let r = rotalar(
        r#"
Rails.application.routes.draw do
  root to: "home#index"
  resources :photos, path: "images", param: :slug, only: :show
  resources :magazines, only: [] do
    resources :ads, only: %i[index show], shallow: true
    get :preview, on: :member
    get "stats"
  end
  namespace :admin, path: "yonetim" do
    resources :reports, controller: "raporlar", except: %i[new edit create update destroy]
  end
  scope module: :v2 do
    resource :account, only: :show do
      resources :tokens, only: :index
    end
  end
  controller :sessions do
    get "login" => :new
  end
  get "photos/search"
  get "photos/:id/zoom", to: "photos#zoom", constraints: { id: /\d+/ }
  match "ping", to: "health#ping", via: :all
  get "*path", to: "errors#not_found"
end
"#,
        &[],
    );
    let beklenen = [
        "GET / home#index",
        "GET /images/:slug photos#show",
        "GET /magazines/:magazine_id/ads ads#index",
        "GET /ads/:id ads#show",
        "GET /magazines/:id/preview magazines#preview",
        "GET /magazines/:magazine_id/stats magazines#stats",
        "GET /yonetim/reports admin/raporlar#index",
        "GET /yonetim/reports/:id admin/raporlar#show",
        "GET /account v2/accounts#show",
        "GET /account/tokens v2/tokens#index",
        "GET /login sessions#new",
        "GET /photos/search photos#search",
        "GET /photos/:id/zoom photos#zoom",
        "GET /ping health#ping",
        "POST /ping health#ping",
        "PUT /ping health#ping",
        "PATCH /ping health#ping",
        "DELETE /ping health#ping",
        "GET /:path errors#not_found",
    ];
    assert_eq!(r, beklenen);
}

#[test]
fn ruby_davranis_ozeti() {
    let d = dosya(
        "app/controllers/x_controller.rb",
        r#"
class XController < ApplicationController
  def destroy
    post = Post.find_by(id: params[:id])
    raise ActiveRecord::RecordNotFound unless post
    head :forbidden if post.user_id != current_user.id
    durum = params[:kalici] ? :silindi : :cop
    post.update(deleted_at: Time.current)
    CleanupJob.set(wait: 1.hour).perform_later(post.id)
    UserMailer.with(user: current_user).bye.deliver_now
    ActiveSupport::Notifications.instrument("post.silindi", id: post.id)
  end
end
"#,
    );
    let m = &d.siniflar[0].metotlar[0];
    let f = &m.dna.firlatmalar;
    assert_eq!(f[0].durum, Some(404));
    assert_eq!(f[0].kosul.as_deref(), Some("!(post)"));
    assert_eq!(f[1].durum, Some(403));
    assert!(f[1].sahiplik, "post.user_id != current_user.id");
    assert_eq!(m.dna.dallar[0].baglam, "durum");
    assert!(m.dna.yumusak.is_some());
    let t: Vec<(&str, &str)> = m.dna.tetikler.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
    assert_eq!(t, [("job", "CleanupJob"), ("job", "UserMailer.bye"), ("event", "post.silindi")]);
    assert_eq!(m.atamalar[0].0, "post");
}

#[test]
fn derin_ic_ice_girdi_yigini_tasirmaz() {
    // Ayrıştırma iş parçacıklarının (rayon) yığın boyutunda: özyinelemeli okuyucular derinlik sınırında durmalı
    let h = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let dizi = format!("x = {}1{}\n", "[".repeat(20_000), "]".repeat(20_000));
            let bloklar = format!("{}{}", "a do\n".repeat(5_000), "end\n".repeat(5_000));
            let siniflar = format!("{}{}", "module M\n".repeat(3_000), "end\n".repeat(3_000));
            let kapanmamis = "resources :a, only: [".repeat(5_000);
            for m in [dizi, bloklar, siniflar, kapanmamis] {
                assert!(ayristir_metin("config/routes.rb", &m).is_some());
            }
        })
        .unwrap();
    h.join().unwrap();
}

#[test]
fn sinif_modeli_ve_geri_cagri_bloklari() {
    let d = dosya(
        "app/models/a.rb",
        r#"
module Shop
  class Order < ApplicationRecord
    after_create_commit { NotifyJob.perform_later(id) }
    has_many :items, dependent: :destroy

    def total; end

    private

    def secret; end
  end
end
"#,
    );
    let s = d.siniflar.iter().find(|s| s.ad == "Shop::Order").unwrap();
    assert_eq!(s.ust.as_deref(), Some("ApplicationRecord"));
    let anonim = s.metotlar.iter().find(|m| m.ad.starts_with("after_create_commit@")).unwrap();
    assert_eq!(anonim.dna.tetikler[0].1, "NotifyJob");
    assert!(!s.metotlar.iter().find(|m| m.ad == "total").unwrap().ozel);
    assert!(s.metotlar.iter().find(|m| m.ad == "secret").unwrap().ozel);
    assert_eq!(s.cagrilar.iter().find(|c| c.ad == "has_many").unwrap().anahtar("dependent").and_then(Arg::ad), Some("destroy"));
}
