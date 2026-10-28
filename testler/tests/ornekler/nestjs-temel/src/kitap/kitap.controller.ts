import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { Yollar, SURUM_2_DEGERI } from '../sabitler';
import { KitapOlusturDto, KitapGuncelleDto } from './dto/kitap.dto';
import { Public } from '../public.decorator';
import { Roles } from '../roles.decorator';
import { KitapService } from './kitap.service';

@Controller({ path: Yollar.Kitap, version: SURUM_2_DEGERI })
export class KitapController {
  constructor(private readonly kitaplar: KitapService) {}

  @Public()
  @Get()
  @ApiOperation({ summary: 'Kitapları listele' })
  listele(@Query('ara') ara?: string, @Query('sayfa') sayfa?: number, @Query('etiket') etiket?: string[]) {
    return this.kitaplar.listele();
  }

  @Post()
  @ApiOperation({ summary: 'Kitap oluştur' })
  olustur(@Body() dto: KitapOlusturDto) {
    return this.kitaplar.olustur(dto);
  }

  @Patch(':id')
  guncelle(@Param('id') id: string, @Body() dto: KitapGuncelleDto) {
    return this.kitaplar.guncelle(id, dto);
  }

  @Roles('ADMIN')
  @Delete(':id')
  sil(@Param('id') id: string) {
    return this.kitaplar.sil(id);
  }

  @Get([':id', ':id/detay'])
  @ApiOperation({ summary: 'Kitap ayrıntısı', deprecated: true })
  getir(@Param('id') id: string) {}
}
