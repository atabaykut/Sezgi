import { Body, Controller, Delete } from '@nestjs/common';
import { AssetService } from '../services/asset.service';

@Controller('assets')
export class AssetController {
  constructor(private readonly service: AssetService) {}

  @Delete()
  deleteAssets(@Body() dto: { ids: string[]; force?: boolean }) {
    return this.service.deleteAll(dto.ids, dto.force);
  }
}
