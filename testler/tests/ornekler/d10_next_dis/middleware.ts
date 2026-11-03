import { NextResponse, type NextRequest } from "next/server";
import { getToken } from "next-auth/jwt";

// Sepet oturum ister; ürünler ve giriş açık kalır
export async function middleware(req: NextRequest) {
  const token = await getToken({ req });
  if (!token) return NextResponse.json({ hata: "giriş gerekli" }, { status: 401 });
  return NextResponse.next();
}

export const config = {
  matcher: ["/api/sepet/:path*"],
};
