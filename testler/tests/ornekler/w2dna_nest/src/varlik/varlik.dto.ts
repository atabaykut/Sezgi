import { IsBoolean, IsOptional, IsString } from 'class-validator';

export class TopluSilDto {
  @IsString({ each: true })
  ids!: string[];

  @IsOptional()
  @IsBoolean()
  force?: boolean;
}
