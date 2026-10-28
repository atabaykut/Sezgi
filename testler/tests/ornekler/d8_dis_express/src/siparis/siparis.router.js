const { Router } = require('express');
const servis = require('./siparis.service');

const router = Router();

router.post('/siparisler', async (req, res) => {
  res.json(await servis.siparisVer(req.body));
});

router.get('/ucuslar/:no', async (req, res) => {
  res.json(await servis.ucusGetir(req.params.no));
});

router.delete('/siparisler/:id', async (req, res) => {
  await servis.siparisSil(req.params.id);
  res.sendStatus(204);
});

router.put('/siparisler/:id', async (req, res) => {
  res.json(await servis.siparisGuncelle(req.params.id, req.body));
});

router.post('/notlar', async (req, res) => {
  res.json(await servis.notEkle(req.body));
});

router.delete('/notlar/:id', async (req, res) => {
  await servis.notSil(req.params.id);
  res.sendStatus(204);
});

router.post('/notlar/:id/restore', async (req, res) => {
  res.json(await servis.notGeriYukle(req.params.id));
});

router.post('/yansit', async (req, res) => {
  res.json(await servis.yansit(req.body.adres));
});

router.post('/notlar/:id/flag', async (req, res) => {
  res.json(await servis.notIsaretle(req.params.id));
});

router.post('/notlar/:id/unflag', async (req, res) => {
  res.json(await servis.notIsaretKaldir(req.params.id));
});

router.put('/notlar/:id', async (req, res) => {
  res.json(await servis.notGuncelle(req.params.id, req.body));
});

router.post('/notlar/:id/revisions/:surum/restore', async (req, res) => {
  res.json(await servis.notSurumuneDon(req.params.id, req.params.surum));
});

module.exports = router;
