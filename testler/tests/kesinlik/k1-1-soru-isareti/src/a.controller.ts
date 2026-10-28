import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import { JwtAuthGuard } from './jwt-auth.guard';
import { Girdi } from './girdi.dto';

@Controller('a')
@UseGuards(JwtAuthGuard)
export class AController {
  @Post()
  yaz(@Body() dto: Girdi) {}
}
