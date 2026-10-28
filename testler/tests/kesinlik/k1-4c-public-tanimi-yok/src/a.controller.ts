import { Controller, Get } from '@nestjs/common';
import { Public } from '@acme/kimlik';

// Public ve guard derlenmiş bir kütüphaneden: guard'ın bu işareti okuduğu koddan doğrulanamaz
@Controller('a')
export class AController {
  @Public()
  @Get()
  liste() {}
}
