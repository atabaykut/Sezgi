import { Controller, Get, UseGuards } from '@nestjs/common';
import { OptionalJwtGuard, Public } from './guards';

@Controller('a')
export class AController {
  // Genel JwtAuthGuard önce çalışır ve oturumsuz isteği reddeder: metottaki isteğe bağlı guard bunu açmaz
  @UseGuards(OptionalJwtGuard)
  @Get('karisik')
  karisik() {}

  // Genel guard işareti okuyup atlanır; yalnızca isteğe bağlı guard kalır
  @Public()
  @UseGuards(OptionalJwtGuard)
  @Get('acik-istege')
  acikIstege() {}
}
