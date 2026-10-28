const express = require('express');
const { getArticle, deleteArticle } = require('../services/article');

const router = express.Router();

router.get('/articles/:slug', async (req, res) => {
  const article = await getArticle(req.params.slug);
  res.json({ article });
});

router.delete(
  '/articles/:slug',
  async (req, res) => {
    await deleteArticle(req.params.slug, req.user.id);
    res.sendStatus(204);
  },
);

module.exports = router;
