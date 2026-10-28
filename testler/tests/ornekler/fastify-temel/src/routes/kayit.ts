import { requireAuth } from '../auth.js';

const korumali = { preHandler: requireAuth };

export function kayitRotalari(app) {
  app.post('/kayit', {
    schema: {
      body: {
        type: 'object',
        required: ['eposta', 'sifre'],
        properties: {
          eposta: { type: 'string', format: 'email' },
          sifre: { type: 'string', minLength: 8 },
          dil: { type: 'string', enum: ['tr', 'en'] },
        },
      },
    },
  }, async (request, reply) => ({ ok: true }));
  app.get('/profil', korumali, async (request) => ({}));
}
