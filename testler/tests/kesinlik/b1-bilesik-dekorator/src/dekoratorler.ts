import { applyDecorators, SetMetadata, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { AccessGuard, ScopeGuard } from './guardlar';

export const KAPSAM = 'kapsam';

// Giriş guard'ını bileşik dekoratörle uygular (Ghostfolio @RequiresScope kalıbı)
export function KapsamIste(...kapsamlar: string[]) {
  return applyDecorators(SetMetadata(KAPSAM, kapsamlar), UseGuards(AuthGuard('jwt'), ScopeGuard));
}

// Kimliği olmayan, erişim anahtarıyla çalışan uç: guard'lar tanınmıyor, kesin "herkese açık" denemez
export function ErisimIle(kapsam: string) {
  return applyDecorators(SetMetadata(KAPSAM, [kapsam]), UseGuards(AccessGuard, ScopeGuard));
}

// İç içe: bileşik dekoratör başka bir bileşik dekoratörü uygular
export const KimlikIste = () => UseGuards(AuthGuard('jwt'));
export const Korumali = () => applyDecorators(KimlikIste());

// Koşullu: guard her zaman uygulanmaz; kesin "giriş gerekli" denemez
export function Belki(acik: boolean) {
  if (acik) {
    return UseGuards(AuthGuard('jwt'));
  }
  return applyDecorators();
}
