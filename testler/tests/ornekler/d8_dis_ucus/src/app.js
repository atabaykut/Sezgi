const express = require('express');

const app = express();
app.get('/flights/:no', (req, res) => {
  res.json({ no: req.params.no, durum: 'zamaninda' });
});
app.listen(3001);
