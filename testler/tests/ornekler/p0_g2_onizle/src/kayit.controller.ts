import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { IsString, Matches } from 'class-validator';
import { D1, D2, D3, D4, D5, D6, D7 } from './desenler';

// Her alanın kuralı başka bir dosyadaki sabit desen: önizlemede hiçbiri sınanamaz (7 ayrı "kontrol edilemedi" nedeni)
export class KayitDto {
  @IsString() @Matches(D1) a!: string;
  @IsString() @Matches(D2) b!: string;
  @IsString() @Matches(D3) c!: string;
  @IsString() @Matches(D4) d!: string;
  @IsString() @Matches(D5) e!: string;
  @IsString() @Matches(D6) f!: string;
  @IsString() @Matches(D7) g!: string;
}

@Controller('kayitlar')
@UseGuards(AuthGuard('jwt'))
export class KayitController {
  @Post()
  olustur(@Body() dto: KayitDto) {}
}
