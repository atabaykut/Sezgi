import { ExecutionContext, Injectable } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { AuthGuard } from '@nestjs/passport';

// Guard 'isPublicRoute' anahtarını okuyor; @Public() ise 'isPublic' yazıyor: işaret etkisiz, giriş gerekir
@Injectable()
export class JwtAuthGuard extends AuthGuard('jwt') {
  constructor(private readonly reflector: Reflector) {
    super();
  }

  canActivate(context: ExecutionContext) {
    if (this.reflector.get<boolean>('isPublicRoute', context.getHandler())) {
      return true;
    }
    return super.canActivate(context);
  }
}
