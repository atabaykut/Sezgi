import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { IsArray, IsInt, IsString } from 'class-validator';
import { z } from 'zod';
import { createZodDto } from 'nestjs-zod';

export class Liste {
  @IsArray() @IsInt({ each: true }) ids!: number[];
  @IsArray() @IsString({ each: true }) etiketler!: string[];
}

const Sema = z.object({
  adlar: z.string().array(),
  deger: z.string().or(z.number()),
  kisa: z.string().min(2).array().min(1),
});
export class ZodGirdi extends createZodDto(Sema) {}

@UseGuards(AuthGuard('jwt'))
@Controller('a')
export class AController {
  @Post('liste') liste(@Body() dto: Liste) {}
  @Post('zod') zod(@Body() dto: ZodGirdi) {}
}
