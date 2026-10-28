const express = require('express');
const db = require('../db');
const { requireAuth, requireRole } = require('../auth');

const router = express.Router();

// Delete a user and, through ON DELETE CASCADE, all of their orders (admins only)
router.delete('/:id', requireAuth, requireRole('admin'), (req, res) => {
  const result = db.run('DELETE FROM users WHERE id = ?', [Number(req.params.id)]);
  if (result.changes === 0) {
    return res.status(404).json({ error: 'User not found' });
  }
  res.status(204).end();
});

module.exports = router;
