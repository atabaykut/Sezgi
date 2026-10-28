import { IsOptional, IsString } from 'class-validator';
import { Kosullu, SecimliMetin } from './dogrulama';

export class Girdi {
  // TypeScript'teki ? var ama IsOptional yok: class-validator undefined değeri reddeder, alan zorunludur
  @IsString() ad?: string;
  @IsOptional() @IsString() not?: string;
  // Hiç doğrulayıcı yok: ValidationPipe bu alanın varlığını denetlemez; zorunluluk koddan çıkarılamaz
  serbest?: string;
  ham: string;
  @SecimliMetin() takma?: string;
  @Kosullu({ bos: true }) kosul?: string;
}
