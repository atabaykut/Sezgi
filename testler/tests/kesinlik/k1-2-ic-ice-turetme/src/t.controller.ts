import { Body, Controller, Post } from '@nestjs/common';
import { A, B, C, D, E, F, G, H } from './t.dto';

@Controller('t')
export class TController {
  @Post('a') a(@Body() dto: A) {}
  @Post('b') b(@Body() dto: B) {}
  @Post('c') c(@Body() dto: C) {}
  @Post('d') d(@Body() dto: D) {}
  @Post('e') e(@Body() dto: E) {}
  @Post('f') f(@Body() dto: F) {}
  @Post('g') g(@Body() dto: G) {}
  @Post('h') h(@Body() dto: H) {}
}
