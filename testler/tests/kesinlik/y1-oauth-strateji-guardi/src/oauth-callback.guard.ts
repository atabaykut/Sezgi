import { AuthGuard } from '@nestjs/passport';

// Strateji adı alan guard üreticisi (Ghostfolio kalıbı): geri dönüş ucu girişi tamamlar, oturum istemez
export function OAuthCallbackGuard(strategy: string) {
  class OAuthCallbackGuardMixin extends AuthGuard(strategy) {}
  return OAuthCallbackGuardMixin;
}
