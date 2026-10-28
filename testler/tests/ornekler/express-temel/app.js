const express = require('express');
const { authMiddleware } = require('./middleware/auth');
const kitaplar = require('./routes/kitaplar');
const yonetimRouter = require('./routes/yonetim');
const zincir = require('./routes/zincir');

const app = express();
app.use(express.json());

app.get('/saglik', (req, res) => res.json({ ok: true }));
app.use('/api/kitaplar', kitaplar);
// Bütün yönetim rotaları girişli
app.use('/api/yonetim', authMiddleware, yonetimRouter);
app.use(zincir);
app.use('/paylasim', require('./routes/paylasim'));

app.listen(3000);
