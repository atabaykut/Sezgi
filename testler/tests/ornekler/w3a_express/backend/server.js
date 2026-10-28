import express from 'express';
import kullaniciRoutes from './routes/kullaniciRoutes.js';

const app = express();
app.use('/api/kullanicilar', kullaniciRoutes);
app.listen(5000);
