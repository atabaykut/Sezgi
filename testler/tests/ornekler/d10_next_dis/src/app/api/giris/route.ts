export async function POST(req: Request) {
  const { eposta, sifre } = await req.json();
  const dogru = await sifreDogrula(sifre);
  // Yanlış şifrede 401: bu uç girişin kendisidir, giriş gerektirmez
  if (!dogru) return new Response("Unauthorized", { status: 401 });
  return Response.json({ ok: true });
}
