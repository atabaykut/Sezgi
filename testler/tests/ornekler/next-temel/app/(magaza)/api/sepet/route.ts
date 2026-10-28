export async function POST(req: Request) {
  const govde = await req.json();
  return Response.json({ urun: govde.urunId, adet: govde.adet });
}
