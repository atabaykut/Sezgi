import { Controller, Get, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';

// Dosya adı ".controller.ts" ile bitmiyor ama @Controller dekoratörlü: haritaya girmeli
@Controller('yonetim')
@UseGuards(AuthGuard('jwt'))
export class YonetimController {
  @Get('durum')
  durum() {}
}
