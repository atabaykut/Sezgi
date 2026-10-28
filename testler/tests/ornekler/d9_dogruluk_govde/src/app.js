import express from 'express';
import kullanicilar from './kullanicilar.js';

const app = express();
app.use(express.json());
app.use('/api/kullanicilar', kullanicilar);
app.listen(3000);
