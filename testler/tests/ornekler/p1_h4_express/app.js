const express = require('express');
const urunler = require('./routes/urunler');

const app = express();
app.use(express.json());
app.use('/api/urunler', urunler);
app.listen(3000);
