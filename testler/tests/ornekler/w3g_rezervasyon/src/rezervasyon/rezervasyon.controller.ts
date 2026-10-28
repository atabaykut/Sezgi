import { Body, Controller, Get, Param, Patch, Query } from '@nestjs/common';
import { RezervasyonGuncelleDto } from './dto/rezervasyon.dto';
import { RezervasyonService } from './rezervasyon.service';

@Controller('rezervasyonlar')
export class RezervasyonController {
  constructor(private readonly rezervasyonlar: RezervasyonService) {}

  @Get()
  @ApiOperation({ summary: 'Varış tarihine göre rezervasyonlar' })
  listele(@Query('varis') varis?: string) {
    return this.rezervasyonlar.listele(varis);
  }

  @Get(':rezervasyonNo')
  @ApiOperation({ summary: 'Rezervasyon numarasıyla rezervasyon' })
  getir(@Param('rezervasyonNo') rezervasyonNo: string) {
    return this.rezervasyonlar.getir(rezervasyonNo);
  }

  @Patch(':id')
  @ApiOperation({ summary: 'Rezervasyonun transfer notunu güncelle' })
  guncelle(@Param('id') id: string, @Body() dto: RezervasyonGuncelleDto) {
    return this.rezervasyonlar.guncelle(id, dto);
  }
}
