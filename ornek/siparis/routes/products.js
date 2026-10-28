const express = require('express');
const db = require('../db');

const router = express.Router();

// List products (public)
router.get('/', (req, res) => {
  const products = db.all('SELECT id, name, price_cents FROM products ORDER BY id');
  res.json({ products });
});

module.exports = router;
