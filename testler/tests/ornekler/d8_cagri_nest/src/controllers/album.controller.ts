import { Body, Controller, Delete, Get, Param, Patch, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { AdDto } from '../dtos/ad.dto';
import { AlbumService } from '../services/album.service';

@UseGuards(AuthGuard('jwt'))
@Controller('albums')
export class AlbumController {
  constructor(private service: AlbumService) {}

  @Get(':id')
  getAlbumInfo(@Param('id') id: string) {
    return this.service.get({ user: { id: 'u' } }, id);
  }

  @Delete(':id')
  deleteAlbum(@Param('id') id: string) {
    return this.service.delete({ user: { id: 'u' } }, id);
  }

  @Patch(':id')
  renameAlbum(@Param('id') id: string, @Body() dto: AdDto) {
    return this.service.rename({ user: { id: 'u' } }, id, dto.ad);
  }

  @Post(':id/copy')
  copyAlbum(@Param('id') id: string) {
    return this.service.copy(id);
  }
}
