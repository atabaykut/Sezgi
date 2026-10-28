import { Controller, Get } from '@nestjs/common';
import { Public } from './public.decorator';

@Controller('a')
export class AController {
  @Public()
  @Get()
  liste() {}
}
