import { Body, Controller, Post } from '@nestjs/common';
import { ApiProperty as Belge } from '@nestjs/swagger';
import { IsOptional as Secimli, IsString } from 'class-validator';

export class Girdi {
  // Secimli = IsOptional (takma ad): alan isteğe bağlıdır, "zorunlu" kesin olamaz
  @Secimli() @IsString() not?: string;
  // Belge = ApiProperty (takma ad): doğrulamaya etkisi yok; IsString alanı zorunlu yapar
  @Belge() @IsString() ad!: string;
}

@Controller('a')
export class AController {
  @Post()
  yaz(@Body() dto: Girdi) {}
}
