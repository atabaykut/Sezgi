use super::*;

#[test]
fn sahiplik_yalnizca_sahip_alani_ile_istek_kimligi() {
    // Kaydın sahip alanı ile istekteki kimlik: sahiplik (kimlik açıkça istekten)
    assert_eq!(sahiplik_kimligi("item.owner_id != current_user.id", None), Some(true));
    assert_eq!(sahiplik_kimligi("not current_user.is_superuser and (item.owner_id != current_user.id)", Some(403)), Some(true));
    assert_eq!(sahiplik_kimligi("album.ownerId !== auth.user.id", None), Some(true));
    assert_eq!(sahiplik_kimligi("$post->user_id !== $request->user()->id", None), Some(true));
    assert_eq!(sahiplik_kimligi("@article.user_id != @current_user.id", None), Some(true));
    assert_eq!(sahiplik_kimligi("note.getOwnerId() == currentUserId()", None), Some(true));
    // İstek kimliği yalnızca adından: inferred
    assert_eq!(sahiplik_kimligi("article.authorId !== userId", None), Some(false));
    assert_eq!(sahiplik_kimligi("existingArticle.author.id !== id", Some(403)), Some(false));
    // 409 benzersizlik: kaydın kimliği yol parametresiyle; sahiplik değil
    assert_eq!(sahiplik_kimligi("existing_user and existing_user.id != user_id", Some(409)), None);
    // "Kendi hesabını silemezsin": çağıranın kendisiyle karşılaştırma; sahiplik değil
    assert_eq!(sahiplik_kimligi("auth.user.id === id", Some(403)), None);
    // Yalın `id` ancak 401 / 403 / 404 ile reddedilen koşulda kimliktir
    assert_eq!(sahiplik_kimligi("existingArticle.author.id !== id", None), None);
    // Karşılaştırma yok
    assert_eq!(sahiplik_kimligi("item.owner_id", Some(403)), None);
}

#[test]
fn sql_dizgisi_tam_deyim_ister() {
    assert_eq!(sql_tablolari("DELETE FROM comments WHERE article_id = $1"), vec![("comments".to_string(), true)]);
    assert_eq!(sql_tablolari("UPDATE \"user\" SET name = $1 WHERE id = $2"), vec![("user".to_string(), true)]);
    let v = sql_tablolari("SELECT a.id FROM album a JOIN album_user u ON u.album_id = a.id");
    assert!(v.contains(&("album".to_string(), false)) && v.contains(&("album_user".to_string(), false)), "{v:?}");
    assert_eq!(sql_tablolari("INSERT INTO public.tags (name) VALUES ($1)"), vec![("tags".to_string(), true)]);
    // Düz metin ve belge dizgisi SQL değildir
    assert!(sql_tablolari("Update own user.").is_empty());
    assert!(sql_tablolari("Select the album from the list").is_empty());
    assert!(sql_tablolari("delete this item").is_empty());
}

#[test]
fn tip_parcalama() {
    assert_eq!(tip_parcala("Repository<User>"), ("Repository".to_string(), vec!["User".to_string()]));
    assert_eq!(tip_parcala("JpaRepository<Note, Long>"), ("JpaRepository".to_string(), vec!["Note".to_string(), "Long".to_string()]));
    assert_eq!(tip_parcala("*service.ItemService").0, "ItemService");
    assert_eq!(tip_parcala("App\\Models\\Post").0, "Post");
    assert_eq!(tip_parcala("Annotated[ItemService, Depends()]").0, "ItemService");
    assert_eq!(tip_parcala("Optional[Item]").0, "Item");
    assert_eq!(tip_parcala("IRequestHandler<Edit.Command, ArticleEnvelope>").1, vec!["Command".to_string(), "ArticleEnvelope".to_string()]);
}

#[test]
fn erisim_denetimi_adlari() {
    for (alici, ad) in [
        (vec![], "requireAccess"),
        (vec!["this".to_string()], "checkOwnablePermission"),
        (vec!["this".to_string()], "authorize"),
        (vec![], "can?"),
        (vec!["Gate".to_string()], "allows"),
        (vec![], "canEditArticle"),
        (vec!["user".to_string()], "can"),
    ] {
        assert!(erisim_cagrisi(&alici, ad), "{ad}");
    }
    for (alici, ad) in [(vec![], "can"), (vec![], "check"), (vec![], "getAccessToken"), (vec![], "allows"), (vec![], "save")] {
        assert!(!erisim_cagrisi(&alici, ad), "{ad}");
    }
    assert!(nesne_duzeyi("requireAccess", &["{ auth, permission: Permission.AlbumDelete, ids: [id] }".into()]));
    assert!(nesne_duzeyi("authorize", &["'update'".into(), "$post".into()]));
    assert!(!nesne_duzeyi("checkPermission", &["'settings-manage'".into()]));
}

#[test]
fn orm_islem_adlari() {
    for a in ["save", "deleteFrom", "updateMany", "create", "destroy", "Remove", "AddAsync", "delete_all", "findOneAndUpdate"] {
        assert_eq!(yazma_islemi(a), Some(true), "{a}");
    }
    for a in ["findUnique", "findById", "getById", "count", "Where", "FirstOrDefaultAsync", "filter_by", "all"] {
        assert_eq!(yazma_islemi(a), Some(false), "{a}");
    }
    assert_eq!(yazma_islemi("execute"), None);
}

#[test]
fn tek_dosyada_cagri_grafigi() {
    let kaynak = br#"
import { Injectable } from '@nestjs/common';
export class Depo {
  constructor(private db: any) {}
  sil(id: string) { return this.db.deleteFrom('kayit').where('id', '=', id).execute(); }
}
export class Servis {
  constructor(private depo: Depo) {}
  async sil(auth: any, id: string) {
    const k = await this.depo.sil(id);
    if (k.ownerId !== auth.user.id) { throw new Error('x'); }
    const ad = 'sil';
    return (this.depo as any)[ad](id);
  }
}
"#;
    let oz = ayristir(Path::new("a.ts"), kaynak.to_vec()).unwrap();
    let dosyalar = vec![("src/a.ts".to_string(), &oz)];
    let g = Grafik::kur(&dosyalar, &[], &[]);
    let servis = g.fonksiyonlar.iter().position(|f| f.gorunen() == "Servis.sil").unwrap();
    let a = g.analiz_yontemle(servis, "");
    assert!(a.ulasilan.iter().any(|u| g.fonksiyonlar[u.fonk].gorunen() == "Depo.sil" && u.guven == Guven::Certain));
    assert!(a.tablolar.iter().any(|(t, _)| t.tablo == "kayit" && t.yazma));
    assert!(a.erisim.iter().any(|(e, _)| e.tur == "kosul" && e.kesin));
    assert_eq!(a.dinamik, 1);
}
