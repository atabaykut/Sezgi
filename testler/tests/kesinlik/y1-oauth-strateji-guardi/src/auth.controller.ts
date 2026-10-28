import { Controller, Get, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { OAuthCallbackGuard } from './oauth-callback.guard';

@Controller('auth')
export class AuthController {
  // OAuth girişini başlatır: oturumu olmayan kullanıcı sağlayıcıya yönlendirilir, reddedilmez
  @UseGuards(AuthGuard('google'))
  @Get('google')
  google() {}

  // Kullanıcı adı/parola ile giriş ucu: oturum gerektirmez
  @UseGuards(AuthGuard('local'))
  @Get('giris')
  giris() {}

  // Strateji adı alan guard üreticisi: OAuth geri dönüş ucu, oturum gerektirmez
  @UseGuards(OAuthCallbackGuard('google'))
  @Get('google/callback')
  googleDonus() {}

  // JWT doğrulaması: oturum gerekir
  @UseGuards(AuthGuard('jwt'))
  @Get('ben')
  ben() {}
}
