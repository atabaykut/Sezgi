const express = require('express');
const { z } = require('zod');
const db = require('../db');
const { requireAuth } = require('../auth');
const { refundPayment } = require('../services/payments');
const { sendReceipt } = require('../services/mailer');

const router = express.Router();

const createOrderSchema = z.object({
  productId: z.number().int().positive(),
  quantity: z.number().int().min(1).max(10),
});

// Show one of my orders
router.get('/:id', requireAuth, (req, res) => {
  const order = db.get('SELECT id, user_id, product_id, quantity, total_cents, status FROM orders WHERE id = ?', [Number(req.params.id)]);
  if (!order || order.user_id !== req.user.id) {
    return res.status(404).json({ error: 'Order not found' });
  }
  res.json({ order });
});

// Place an order
router.post('/', requireAuth, (req, res) => {
  let input;
  try {
    input = createOrderSchema.parse(req.body);
  } catch (err) {
    return res.status(400).json({ error: 'Invalid order', details: err.errors });
  }
  const product = db.get('SELECT id, price_cents FROM products WHERE id = ?', [input.productId]);
  if (!product) {
    return res.status(404).json({ error: 'Product not found' });
  }
  const total = product.price_cents * input.quantity;
  const result = db.run('INSERT INTO orders (user_id, product_id, quantity, total_cents, status) VALUES (?, ?, ?, ?, ?)',
    [req.user.id, product.id, input.quantity, total, 'paid']);
  res.status(201).json({ id: result.lastId, total_cents: total, status: 'paid' });
});

// Refund an order through the payment provider
router.post('/:id/refund', requireAuth, async (req, res) => {
  const order = db.get('SELECT id, user_id, total_cents, status FROM orders WHERE id = ?', [Number(req.params.id)]);
  if (!order || order.user_id !== req.user.id) {
    return res.status(404).json({ error: 'Order not found' });
  }
  if (order.status === 'refunded') {
    return res.status(409).json({ error: 'Order already refunded' });
  }
  const refund = await refundPayment(order.id, order.total_cents);
  db.run("UPDATE orders SET status = 'refunded' WHERE id = ?", [order.id]);
  res.json({ id: order.id, status: 'refunded', refund_id: refund.id });
});

// E-mail the receipt of an order
router.post('/:id/email-receipt', requireAuth, async (req, res) => {
  const order = db.get('SELECT id, user_id, product_id, quantity, total_cents FROM orders WHERE id = ?', [Number(req.params.id)]);
  if (!order || order.user_id !== req.user.id) {
    return res.status(404).json({ error: 'Order not found' });
  }
  const info = await sendReceipt(req.user.email, order);
  res.json({ sent: true, message_id: info.messageId });
});

module.exports = router;
