const express = require('express');
const multer = require('multer');
const { protect, admin } = require('../middleware/auth');

const router = express.Router();
const upload = multer({ dest: 'uploads/' });

router.get('/', (req, res) => res.json([]));
// Yalnızca yönetici silebilir (proshop deseni: protect, admin)
router.delete('/:id', protect, admin, (req, res) => res.json({ id: req.params.id }));
// Dosya yüklemesi (multer): dosya gövde şemasında değildir
router.post('/resim', protect, upload.single('resim'), (req, res) => {
  const { aciklama } = req.body;
  res.json({ yol: req.file.path, aciklama });
});

module.exports = router;
