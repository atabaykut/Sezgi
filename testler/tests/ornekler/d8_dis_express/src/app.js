const express = require('express');
const siparisler = require('./siparis/siparis.router');

const app = express();
app.use(express.json());
app.use('/api', siparisler);
app.listen(3000);
