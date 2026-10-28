import { Controller, Get, Param } from '@nestjs/common';
import { RezervasyonService } from './rezervasyon.service';

@Controller('misafirler')
export class MisafirController {
  constructor(private readonly rezervasyonlar: RezervasyonService) {}

  @Get(':id')
  @ApiOperation({ summary: 'Misafir' })
  getir(@Param('id') id: string) {
    return this.rezervasyonlar.misafir(id);
  }
}
