const express = require('express');

const app = express();
app.get('/api/notes', (req, res) => res.json([]));
app.post('/api/notes', (req, res) => res.status(201).json({}));
app.get('/api/notes/:id', (req, res) => res.json({}));
app.delete('/api/notes/:id', (req, res) => res.sendStatus(204));
app.post('/api/notes/:id/like', (req, res) => res.json({}));
app.listen(4000);
