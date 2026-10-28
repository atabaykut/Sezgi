import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { Roles } from '../roles.decorator';
import { TransferGuncelleDto, TransferOlusturDto } from './dto/transfer.dto';
import { TransferService } from './transfer.service';

@Controller('transferler')
export class TransferController {
  constructor(private readonly transferler: TransferService) {}

  @Get()
  @ApiOperation({ summary: 'Rezervasyona göre transferler' })
  listele(@Query('rezervasyonRef') rezervasyonRef?: string) {
    return this.transferler.listele(rezervasyonRef);
  }

  @Post()
  @ApiOperation({ summary: 'Transfer oluştur' })
  olustur(@Body() dto: TransferOlusturDto) {
    return this.transferler.olustur(dto);
  }

  @Roles('OPERASYON')
  @Patch(':id')
  @ApiOperation({ summary: 'Transferin aracını ya da yolcu sayısını değiştir' })
  guncelle(@Param('id') id: string, @Body() dto: TransferGuncelleDto) {
    return this.transferler.guncelle(id, dto);
  }
}
