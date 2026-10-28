const prisma = require('../prisma');
const db = require('../db');

async function getArticle(slug) {
  return prisma.article.findUnique({ where: { slug } });
}

async function deleteArticle(slug, userId) {
  const article = await prisma.article.findUnique({ where: { slug } });
  if (article.authorId !== userId) {
    throw new Error('forbidden');
  }
  await db.query('DELETE FROM comments WHERE article_id = $1', [article.id]);
  const islem = 'delete';
  return prisma.article[islem]({ where: { slug } });
}

module.exports = { getArticle, deleteArticle };
