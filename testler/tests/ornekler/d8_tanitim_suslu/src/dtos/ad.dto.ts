import { IsString } from 'class-validator';

export class AdDto {
  @IsString()
  ad!: string;
}
