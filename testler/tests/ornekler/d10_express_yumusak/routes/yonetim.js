const { Router } = require('express');
const router = Router();

router.post('/temizle', (req, res) => {
  db.prepare('UPDATE kitaplar SET silindi = 1').run();
});

module.exports = router;
