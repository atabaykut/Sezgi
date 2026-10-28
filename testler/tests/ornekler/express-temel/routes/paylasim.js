// Directus holdout'undan (kanit/SONUC-16): Joi şeması, isteği okuyan yardımcı fonksiyon, isteğe bağlı yol parametresi
const express = require('express');
const Joi = require('joi');

const router = express.Router();

const girisSema = Joi.object({
  share: Joi.string().required(),
  password: Joi.string(),
  mode: Joi.string().valid('cookie', 'json').optional(),
}).unknown();

function kipBul(req) {
  if (req.body.kip) return req.body.kip;
  return 'json';
}

router.post('/giris', (req, res) => {
  const { error } = girisSema.validate(req.body);
  if (error) return res.status(400).end();
  res.json({ kip: kipBul(req) });
});

function yanitla(req, res) {
  res.json(res.locals.sonuc);
}

// Cevabı yazan ara katman işleyiciden sonra gelir (Directus: respond)
router.post('/cikis', (req, res, next) => {
  res.locals.sonuc = { kip: kipBul(req) };
  next();
}, yanitla);

router.get('/dosya/:id/:ad?', (req, res) => res.end());

module.exports = router;
