const express = require('express');
const router = express.Router();

// ============================================================
// 2. TEKLİF DÖNÜŞÜM RAPORU
// ============================================================
router.get('/donusum', async (req, res) => {
  res.json([]);
});

// ------------------------
router.post('/:id/onayla', async (req, res) => {
  res.json({});
});

/* ************************
 * 3) Satış özeti
 * ************************ */
router.get('/satis', async (req, res) => {
  res.json([]);
});

// Tek satırlık özet
router.get('/tek', async (req, res) => {
  res.json([]);
});

module.exports = router;
