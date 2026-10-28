import { Body, Controller, Get, Post } from '@nestjs/common';
import { SiparisGirdisi, SiparisV1Dto } from './siparis.dto';

@Controller({ path: 'siparisler', version: '1' })
export class SiparisV1Controller {
  @Get()
  listele() {}

  @Post()
  olustur(@Body() dto: SiparisV1Dto) {}
}

@Controller({ path: 'siparisler', version: '2' })
export class SiparisV2Controller {
  @Get()
  listele() {}

  @Post()
  olustur(@Body() dto: SiparisGirdisi) {}
}
