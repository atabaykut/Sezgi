import { Controller, Get, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';

export enum SayiRol {
  Yonetici = 1,
}

// Sınıf ve metot düzeyinde rol: guard birleşim mi (getAllAndMerge) geçersiz kılma mı (getAllAndOverride) kullanıyor,
// bu depoda görünmüyor; 'editor' rolüyle istek geçebilir de reddedilebilir de
@Roles('editor')
@UseGuards(AuthGuard('jwt'))
@Controller('a')
export class AController {
  @Roles('yonetici')
  @Get('iki')
  iki() {}

  // Sayı değerli enum: rolün metin karşılığı koddan çözülemez
  @Roles(SayiRol.Yonetici)
  @Get('sayi')
  sayi() {}
}
