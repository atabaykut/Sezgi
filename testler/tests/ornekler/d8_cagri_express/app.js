const express = require('express');
const makaleler = require('./routes/article');

const app = express();
app.use(express.json());
app.use('/api', makaleler);

app.listen(3000);
