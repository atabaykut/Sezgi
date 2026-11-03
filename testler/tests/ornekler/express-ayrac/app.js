const express = require('express');
const raporlar = require('./routes/raporlar');

const app = express();
app.use('/api/raporlar', raporlar);
app.listen(3000);
