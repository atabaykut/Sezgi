export default async function urunler(fastify) {
  fastify.get('/', async (request) => {
    const { kategori } = request.query;
    return [];
  });
  fastify.get('/:id', async (request) => ({ id: request.params.id }));
}
