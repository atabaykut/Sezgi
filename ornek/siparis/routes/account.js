const express = require('express');
const { z } = require('zod');
const db = require('../db');
const { requireAuth } = require('../auth');

const router = express.Router();

const changePasswordSchema = z.object({
  currentPassword: z.string().min(1),
  newPassword: z.string().min(12).max(128),
});

// Change my password
router.post('/change-password', requireAuth, (req, res) => {
  let input;
  try {
    input = changePasswordSchema.parse(req.body);
  } catch (err) {
    return res.status(400).json({ error: 'Invalid password', details: err.errors });
  }
  const user = db.get('SELECT id, password_hash FROM users WHERE id = ?', [req.user.id]);
  if (!db.checkPassword(input.currentPassword, user.password_hash)) {
    return res.status(403).json({ error: 'Current password is wrong' });
  }
  db.run('UPDATE users SET password_hash = ? WHERE id = ?', [db.hashPassword(input.newPassword), user.id]);
  res.json({ changed: true });
});

module.exports = router;
