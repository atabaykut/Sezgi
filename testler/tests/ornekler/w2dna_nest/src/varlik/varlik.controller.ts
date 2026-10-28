import { Body, Controller, Delete, Param, Patch } from '@nestjs/common';
import { TopluSilDto } from './varlik.dto';
import { VarlikService } from './varlik.service';

@Controller('varliklar')
export class VarlikController {
  constructor(private readonly service: VarlikService) {}

  @Delete()
  topluSil(@Body() dto: TopluSilDto) {
    return this.service.topluSil('k', dto.ids, dto.force);
  }

  @Delete(':id')
  sil(@Param('id') id: string) {
    return this.service.sil('k', id);
  }

  @Patch(':id')
  adlandir(@Param('id') id: string, @Body() dto: TopluSilDto) {
    return this.service.adlandir(id, 'x');
  }
}
