export async function GET(request: Request, { params }: { params: { id: string } }) {
  const { searchParams } = new URL(request.url);
  const dil = searchParams.get('dil');
  return Response.json({ id: params.id });
}

export async function PUT(request: Request) {
  const session = await getServerSession();
  if (!session) return new Response('Unauthorized', { status: 401 });
  const { ad, fiyat } = await request.json();
  return Response.json({ ad, fiyat });
}
