const prisma = require('../prisma');
const HttpException = require('../hata');

const makaleSil = async (id, kullaniciId) => {
  const makale = await prisma.makale.findFirst({ where: { id } });
  if (!makale) {
    throw new HttpException(404, {});
  }
  if (makale.yazarId !== kullaniciId) {
    throw new HttpException(403, { message: 'not the author' });
  }
  await prisma.makale.delete({ where: { id } });
};

module.exports = { makaleSil };
