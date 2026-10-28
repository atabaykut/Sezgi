import { Controller, Get, SetMetadata, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';

export const ALLOW_DURING_IMPERSONATION_KEY = 'allowDuringImpersonation';
// Kimlikle ilgisiz bir işaret: SetMetadata(KEY, true) yazar ama herkese açıklık değildir
export function AllowDuringImpersonation() {
  return SetMetadata(ALLOW_DURING_IMPERSONATION_KEY, true);
}

@AllowDuringImpersonation()
@Controller('a')
export class AController {
  @Get()
  @UseGuards(AuthGuard('jwt'))
  liste() {}
}
