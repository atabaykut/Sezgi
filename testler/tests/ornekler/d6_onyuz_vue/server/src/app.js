const express = require('express');

const app = express();
app.get('/api/items', (req, res) => res.json([]));
app.post('/api/items', (req, res) => res.status(201).json({}));
app.delete('/api/items/:id', (req, res) => res.sendStatus(204));
app.listen(3000);
