import Stripe from 'stripe';

const stripe = new Stripe(process.env.STRIPE_KEY as string);

export async function POST(req: Request) {
  const { tutar } = await req.json();
  const odeme = await stripe.paymentIntents.create({ amount: tutar, currency: 'try' });
  return Response.json({ id: odeme.id });
}
