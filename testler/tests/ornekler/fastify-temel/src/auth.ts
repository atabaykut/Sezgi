export async function requireAuth(request, reply) {
  if (!request.headers.authorization) {
    reply.code(401).send({ hata: 'Unauthorized' });
  }
}
