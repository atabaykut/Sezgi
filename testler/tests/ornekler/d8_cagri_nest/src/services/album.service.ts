import { ForbiddenException, Injectable, NotFoundException } from '@nestjs/common';
import { Permission } from '../enum';
import { BaseService } from './base.service';

@Injectable()
export class AlbumService extends BaseService {
  async get(auth: any, id: string) {
    await this.requireAccess({ auth, permission: Permission.AlbumRead, ids: [id] });
    const album = await this.albumRepository.getById(id);
    if (!album) {
      throw new NotFoundException('Album not found');
    }
    return album;
  }

  async delete(auth: any, id: string) {
    await this.requireAccess({ auth, permission: Permission.AlbumDelete, ids: [id] });
    await this.albumRepository.delete(id);
  }

  async rename(auth: any, id: string, name: string) {
    const album: any = await this.albumRepository.getById(id);
    if (album.ownerId !== auth.user.id) {
      throw new ForbiddenException('Not the owner');
    }
    return { ...album, albumName: name, isOwner: album.ownerId === auth.user.id };
  }

  async copy(id: string) {
    const yontem = 'getById';
    return (this.albumRepository as any)[yontem](id);
  }
}
