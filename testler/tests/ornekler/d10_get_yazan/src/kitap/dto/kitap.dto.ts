import { IsDateString, IsEnum, IsIn, IsInt, IsOptional, IsString, Matches, Max, MaxLength, Min, MinLength, ValidateIf } from 'class-validator';
import { DILLER, Oncelik, Tur } from '../../sabitler';
import { KitapDurumu } from '@prisma/client';
import { PartialType } from '@nestjs/swagger';

export class KitapOlusturDto {
  @IsString() @MinLength(2) baslik!: string;
  @IsInt() @Min(1) @Max(5000) sayfa!: number;
  @IsOptional() @IsString() yazar?: string;
  @IsEnum(Tur) tur!: Tur;
  @IsOptional() @IsIn(['tr', 'en']) dil?: string;
  @IsOptional() @IsDateString() yayin?: string;
  @IsOptional() @MaxLength(3) kisaAd?: string;
  @IsOptional() @Matches(/^\d+$/) isbn?: string;
  @IsOptional() @IsEnum(Oncelik) oncelik?: Oncelik;
  @ValidateIf((o) => o.dil === 'tr') @IsString() cevirmen?: string;
  @IsOptional() @IsEnum(KitapDurumu) durum?: KitapDurumu;
  @IsOptional() @IsIn(DILLER as unknown as string[]) ikinciDil?: string;
}

export class KitapGuncelleDto extends PartialType(KitapOlusturDto) {}
