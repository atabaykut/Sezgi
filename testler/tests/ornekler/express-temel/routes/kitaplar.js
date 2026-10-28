const express = require('express');
const { z } = require('zod');
const { authMiddleware, requireRole, asyncHandler } = require('../middleware/auth');
const kitapController = require('../controllers/kitap');

const router = express.Router();
const yonetici = requireRole('admin');
const yorumSema = z.object({ metin: z.string().min(1).max(500), puan: z.number().int().optional() });

// Kitapları listele
router.get('/', (req, res) => {
  const { ara, sayfa = 1 } = req.query;
  return db.prepare('SELECT * FROM kitaplar WHERE baslik LIKE ?').all(ara);
});

router.get('/ozet/sayilar', kitapController.ozet);

router.post('/', authMiddleware, async (req, res) => {
  const { baslik, sayfaSayisi = 0, etiketler = [] } = req.body;
  db.prepare('INSERT INTO kitaplar (baslik) VALUES (?)').run(baslik);
});

// Bundan sonraki /:id rotaları girişli
router.use('/:id', authMiddleware);

router.get('/:id', (req, res) => {
  return db.prepare('SELECT * FROM kitaplar k JOIN yazarlar y ON y.id = k.yazar_id WHERE k.id = ?').get(req.params.id);
});

router.delete('/:id', yonetici, asyncHandler(async (req, res) => {
  const neden = req.body.neden;
  db.prepare('DELETE FROM kitaplar WHERE id = ?').run(req.params.id);
}));

router.post('/:id/yorum', async (req, res) => {
  const yorum = yorumSema.parse(req.body);
  db.prepare('INSERT INTO yorumlar (metin) VALUES (?)').run(yorum.metin);
});

router.route('/:id/kapak').put((req, res) => {
  const { url } = req.body;
});

module.exports = router;
