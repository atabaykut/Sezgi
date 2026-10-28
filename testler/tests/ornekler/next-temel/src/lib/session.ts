export async function requireCan(yetki: string) {
  const kullanici = await oturumKullanici();
  if (!kullanici) return { ok: false, res: new Response(null, { status: 401 }) };
  if (!kullanici.yetkiler.includes(yetki)) return { ok: false, res: new Response(null, { status: 403 }) };
  return { ok: true };
}
