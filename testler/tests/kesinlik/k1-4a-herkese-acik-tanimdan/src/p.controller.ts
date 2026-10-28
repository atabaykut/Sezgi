import { Controller, Get, SetMetadata } from '@nestjs/common';
import { AllowAnonymous, Public, SkipAuth } from './herkese-acik';

@Controller('p')
export class PController {
  @Public() @Get('a') a() {}
  @SkipAuth() @Get('b') b() {}
  @AllowAnonymous() @Get('c') c() {}
  @SetMetadata('isPublic', true) @Get('d') d() {}
  // İşaret yok: genel guard giriş ister
  @Get('e') e() {}
  // Başka bir anahtar: guard bunu okumaz, giriş gerekir
  @SetMetadata('onbellek', true) @Get('f') f() {}
}
