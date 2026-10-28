// Receipts are e-mailed with nodemailer. Without SMTP_URL nothing is sent (demo mode).
const nodemailer = require('nodemailer');

async function sendReceipt(to, order) {
  const text = `Order #${order.id}: ${order.quantity} x product ${order.product_id}, total ${(order.total_cents / 100).toFixed(2)}`;
  if (!process.env.SMTP_URL) {
    console.log(`[mailer] demo mode: would e-mail ${to}: ${text}`);
    return { messageId: 'demo' };
  }
  const transporter = nodemailer.createTransport(process.env.SMTP_URL);
  return transporter.sendMail({ from: 'shop@example.com', to, subject: `Receipt for order #${order.id}`, text });
}

module.exports = { sendReceipt };
