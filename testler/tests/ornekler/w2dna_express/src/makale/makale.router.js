const { Router } = require('express');
const { makaleSil } = require('./makale.service');

const router = Router();

router.delete('/makaleler/:id', async (req, res, next) => {
  if (!req.params.id) {
    return res.status(400).json({ hata: 'id' });
  }
  try {
    await makaleSil(req.params.id, req.user.id);
    res.sendStatus(204);
  } catch (e) {
    next(e);
  }
});

module.exports = router;
