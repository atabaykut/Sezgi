import { Controller, Get } from '@nestjs/common';
import { Belki, ErisimIle, KapsamIste, Korumali } from './dekoratorler';

@Controller('hesap')
export class HesapController {
  @Get()
  @KapsamIste('hesap.oku')
  liste() {}

  @Get('erisim')
  @ErisimIle('hesap.oku')
  erisim() {}

  @Get('belki')
  @Belki(true)
  belki() {}
}

@Korumali()
@Controller('ic')
export class IcController {
  @Get()
  ic() {}
}
