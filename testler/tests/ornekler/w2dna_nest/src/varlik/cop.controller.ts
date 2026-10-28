import { Controller, Post } from '@nestjs/common';

@Controller('cop')
export class CopController {
  @Post('geri-yukle/varliklar')
  restoreVarliklar() {
    return [];
  }
}
