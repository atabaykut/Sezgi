import { Body, Controller, Put } from '@nestjs/common';
import { Rota } from './rota';
import { KisilerDto, DurumDto } from './kisi.dto';
import { TercihDto } from './tercih.dto';
import { BelirsizDto } from './belirsiz.dto';

@Controller(Rota.Kisi)
export class KisiController {
  @Put()
  guncelle(@Body() dto: KisilerDto) {}

  @Put('tercihler')
  tercih(@Body() dto: TercihDto) {}

  @Put('belirsiz')
  belirsiz(@Body() dto: BelirsizDto) {}

  @Put('durum')
  durum(@Body() dto: DurumDto) {}
}
