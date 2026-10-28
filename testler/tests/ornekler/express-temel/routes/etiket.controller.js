// Noktalı dosya adı (etiket.controller.js): içe aktarma uzantı eklenerek çözülür
const express = require('express');
const { authMiddleware, optionalAuth } = require('../middleware/auth');

const router = express.Router();

// Etiketleri listele (kimlik varsa kişiye göre sıralanır)
router.get('/etiketler', optionalAuth, (req, res) => res.json([]));

router.post('/etiketler', authMiddleware, (req, res) => {
  const { ad } = req.body;
  res.status(201).json({ ad });
});

module.exports = router;
