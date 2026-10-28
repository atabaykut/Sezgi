import { IsInt, IsOptional, IsString, Min, ValidateNested } from 'class-validator';
import { Type } from 'class-transformer';

export class Adres {
  @IsString() sehir!: string;
  @IsOptional() @IsString() posta?: string;
}

export class Kalem {
  @IsString() urun!: string;
  @IsInt() @Min(1) adet!: number;
}

export class SiparisV1Dto {
  @ValidateNested() @Type(() => Adres) adres!: Adres;
  @ValidateNested({ each: true }) @Type(() => Kalem) kalemler!: Kalem[];
}

export class HizliSiparis {
  @IsString() urun!: string;
}

export class TopluSiparis {
  @ValidateNested({ each: true }) @Type(() => Kalem) kalemler!: Kalem[];
}

// Gövde iki biçimden biri: hangisinin kabul edileceğine uygulamanın kendi kodu karar verir
export type SiparisGirdisi = HizliSiparis | TopluSiparis;
