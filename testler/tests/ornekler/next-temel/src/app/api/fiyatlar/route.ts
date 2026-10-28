import { requireCan } from "@/lib/session";

export async function POST(req: Request) {
  const yetki = await requireCan("FIYAT_YAZ");
  if (!yetki.ok) return yetki.res;
  const { tutar } = await req.json();
  return Response.json({ tutar });
}
