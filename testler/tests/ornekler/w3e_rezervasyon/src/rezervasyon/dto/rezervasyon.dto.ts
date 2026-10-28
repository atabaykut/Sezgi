import { IsOptional, IsString, MaxLength } from 'class-validator';

export class RezervasyonGuncelleDto {
  @IsOptional() @IsString() @MaxLength(200) transferNotu?: string;
}
