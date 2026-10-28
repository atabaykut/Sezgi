const express = require('express');

const app = express();
app.get('/api/heroes', (req, res) => res.json([]));
app.get('/api/heroes/:id', (req, res) => res.json({}));
app.post('/api/heroes', (req, res) => res.status(201).json({}));
app.put('/api/heroes/:id', (req, res) => res.json({}));
app.delete('/api/heroes/:id', (req, res) => res.sendStatus(204));
app.post('/api/auth/login', (req, res) => res.json({}));
app.get('/api/admin/stats', (req, res) => res.json({}));
app.listen(3000);
