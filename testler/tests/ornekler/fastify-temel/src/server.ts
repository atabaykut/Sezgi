import Fastify from 'fastify';
import { kayitRotalari } from './routes/kayit.js';
import urunler from './routes/urunler.js';

const app = Fastify();
kayitRotalari(app);
app.register(urunler, { prefix: '/v1/urunler' });
app.listen({ port: 3000 });
