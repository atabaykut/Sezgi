const express = require('express');

const app = express();
app.delete('/api/varliklar', (req, res) => res.sendStatus(204));
app.post('/api/varliklar', (req, res) => res.status(201).json({}));
app.get('/api/varliklar/:id', (req, res) => res.json({}));
app.listen(3000);
