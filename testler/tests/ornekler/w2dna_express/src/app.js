const express = require('express');
const makaleler = require('./makale/makale.router');

const app = express();
app.use('/api', makaleler);
app.listen(3000);
