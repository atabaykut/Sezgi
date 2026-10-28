// Refunds go to an outside payment provider. Without PAYMENTS_API_KEY nothing is sent (demo mode).
async function refundPayment(orderId, amountCents) {
  if (!process.env.PAYMENTS_API_KEY) {
    console.log(`[payments] demo mode: would refund ${amountCents} cents for order ${orderId}`);
    return { id: 'demo-refund', status: 'skipped' };
  }
  const response = await fetch('https://api.payments.example.com/refunds', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${process.env.PAYMENTS_API_KEY}`,
    },
    body: JSON.stringify({ order_id: orderId, amount_cents: amountCents }),
  });
  if (!response.ok) throw new Error(`payment provider answered ${response.status}`);
  return response.json();
}

module.exports = { refundPayment };
