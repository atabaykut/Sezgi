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

  @Patch(':id')
  @ApiOperation({ summary: 'Rezervasyonun transfer notunu güncelle' })
  guncelle(@Param('id') id: string, @Body() dto: RezervasyonGuncelleDto) {
    return this.rezervasyonlar.guncelle(id, dto);
  }
}
