import { IsInt, IsOptional, IsString, Max, Min } from 'class-validator';

export class TransferOlusturDto {
  @IsString() rezervasyonRef!: string;
  @IsString() yolcuId!: string;
  @IsString() aracId!: string;
  // Araç kapasitesi: bir transferde en çok 8 yolcu
  @IsInt() @Min(1) @Max(8) yolcuSayisi!: number;
}

export class TransferGuncelleDto {
  @IsOptional() @IsString() aracId?: string;
  @IsOptional() @IsInt() @Min(1) @Max(8) yolcuSayisi?: number;
}
