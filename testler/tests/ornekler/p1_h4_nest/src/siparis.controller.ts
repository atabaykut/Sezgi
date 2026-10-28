import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { ApiOperation } from '@nestjs/swagger';
import { IsIn, IsString } from 'class-validator';

// Aynı adlı başka bir dizi a-sabitler.ts içinde var; bu dosya kendi dizisini kullanır
const DURUMLAR = ['acik', 'kapali'];

export class SiparisDto {
  @IsString()
  @IsIn(DURUMLAR)
  durum!: string;
}

@Controller('siparisler')
@UseGuards(AuthGuard('jwt'))
export class SiparisController {
  @Post()
  @ApiOperation({ summary: 'Sipariş oluştur', operationId: 'siparisOlustur' })
  olustur(@Body() dto: SiparisDto) {}
}
