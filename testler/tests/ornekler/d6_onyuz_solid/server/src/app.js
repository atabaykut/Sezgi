const express = require('express');

const app = express();
app.get('/api/todos', (req, res) => res.json([]));
app.post('/api/todos', (req, res) => res.status(201).json({}));
app.delete('/api/todos/:id', (req, res) => res.sendStatus(204));
app.get('/api/users/:id', (req, res) => res.json({}));
app.listen(5000);
