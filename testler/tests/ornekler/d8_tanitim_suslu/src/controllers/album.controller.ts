import { Body, Controller, Delete, Get, Param, Patch, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { AdDto } from '../dtos/ad.dto';
import { AlbumService } from '../services/album.service';
import { Auth } from '../middleware/auth.decorator';

@UseGuards(AuthGuard('jwt'))
@Controller('albums')
export class AlbumController {
  constructor(private service: AlbumService) {}

  @Get(':id')
  getAlbumInfo(@Param('id') id: string) {
    return this.service.get({ user: { id: 'u' } }, id);
  }

  @Delete(':id')
  deleteAlbum(@Auth({ izin: 'album.delete' }) auth: object, @Param('id') id: string) {
    return this.service.delete({ user: { id: 'u' } }, id);
  }

  @Patch(':id')
  renameAlbum(@Param('id') id: string, @Body() dto: AdDto) {
    return this.service.rename({ user: { id: 'u' } }, id, dto.ad);
  }

  @Get('config/defaults')
  getConfigDefaults() {
    return this.service.get({ user: { id: 'u' } }, 'varsayilan');
  }

  @Get('orders/mine')
  getMyOrders() {
    return this.service.get({ user: { id: 'u' } }, 'benim');
  }

  @Get(':id/video/playlist.m3u8')
  getPlaylist(@Param('id') id: string) {
    return this.service.get({ user: { id: 'u' } }, id);
  }

  @Post(':id/copy')
  copyAlbum(@Param('id') id: string) {
    return this.service.copy(id);
  }
}
