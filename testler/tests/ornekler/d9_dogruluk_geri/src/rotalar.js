const { Router } = require('express');
const db = require('./db');

const router = Router();

// Toplu güncelleme ve toplu silme aynı koleksiyon yolunda: biri diğerinin tersi değildir
async function kisileriGuncelle(req, res) {
  for (const k of req.body.kisiler) await db.kisi.update({ where: { id: k.id }, data: k });
  res.json({ ok: true });
}
async function kisileriSil(req, res) {
  await db.kisi.deleteMany({ where: { id: { in: req.body.ids } } });
  res.sendStatus(204);
}
router.put('/kisiler', kisileriGuncelle);
router.delete('/kisiler', kisileriSil);

async function varliklariGuncelle(req, res) {
  await db.varlik.updateMany({ where: { id: { in: req.body.ids } }, data: req.body });
  res.json({ ok: true });
}
async function varliklariSil(req, res) {
  await db.varlik.deleteMany({ where: { id: { in: req.body.ids } } });
  res.sendStatus(204);
}
router.patch('/varliklar', varliklariGuncelle);
router.delete('/varliklar', varliklariSil);

async function beniGuncelle(req, res) {
  res.json(await db.kullanici.update({ where: { id: req.user.id }, data: req.body }));
}
async function beniSil(req, res) {
  await db.kullanici.delete({ where: { id: req.user.id } });
  res.sendStatus(204);
}
router.patch('/kullanicilar/me', beniGuncelle);
router.delete('/kullanicilar/me', beniSil);

// Alt koleksiyona ekleme ve aynı yoldan çıkarma: gerçek ters işlem
async function albumeEkle(req, res) {
  res.json(await db.albumVarlik.createMany({ data: req.body.ids.map((v) => ({ albumId: req.params.id, varlikId: v })) }));
}
async function albumdenCikar(req, res) {
  await db.albumVarlik.deleteMany({ where: { albumId: req.params.id, varlikId: { in: req.body.ids } } });
  res.sendStatus(204);
}
router.put('/albumler/:id/varliklar', albumeEkle);
router.delete('/albumler/:id/varliklar', albumdenCikar);

// Oluşturma: koleksiyonun toplu silmesi tersi değildir, tek kaydı silen uç tersidir
async function oturumOlustur(req, res) {
  res.json(await db.oturum.create({ data: { kullaniciId: req.user.id } }));
}
async function oturumlariSil(req, res) {
  await db.oturum.deleteMany({ where: { kullaniciId: req.user.id } });
  res.sendStatus(204);
}
async function oturumSil(req, res) {
  await db.oturum.delete({ where: { id: req.params.id } });
  res.sendStatus(204);
}
router.post('/oturumlar', oturumOlustur);
router.delete('/oturumlar', oturumlariSil);
router.delete('/oturumlar/:id', oturumSil);

async function etiketOlustur(req, res) {
  res.json(await db.etiket.create({ data: req.body }));
}
async function etiketleriSil(req, res) {
  await db.etiket.deleteMany({ where: { id: { in: req.body.ids } } });
  res.sendStatus(204);
}
router.post('/etiketler', etiketOlustur);
router.delete('/etiketler', etiketleriSil);

// Tekil kaynak: kurma ↔ silme terstir; değiştirme ise önceki değeri geri getirmez
async function pinKur(req, res) {
  res.json(await db.pin.create({ data: { kullaniciId: req.user.id, kod: req.body.kod } }));
}
async function pinDegistir(req, res) {
  res.json(await db.pin.update({ where: { kullaniciId: req.user.id }, data: { kod: req.body.yeni } }));
}
async function pinSifirla(req, res) {
  await db.pin.delete({ where: { kullaniciId: req.user.id } });
  res.sendStatus(204);
}
router.post('/pin-kodu', pinKur);
router.put('/pin-kodu', pinDegistir);
router.delete('/pin-kodu', pinSifirla);

// Kayda işaret: işaretle ↔ işareti kaldır
router.post('/profiller/:ad/takip', async (req, res) => {
  res.json(await db.takip.create({ data: { takipEden: req.user.id, takipEdilen: req.params.ad } }));
});
router.delete('/profiller/:ad/takip', async (req, res) => {
  await db.takip.deleteMany({ where: { takipEden: req.user.id, takipEdilen: req.params.ad } });
  res.sendStatus(204);
});

module.exports = router;
