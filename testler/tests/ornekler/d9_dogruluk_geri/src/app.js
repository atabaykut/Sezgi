const express = require('express');
const r = require('./rotalar');

const app = express();
app.use(express.json());
app.use('/api', r);
app.listen(3000);
