import { Module } from '@nestjs/common';
import { AlbumController } from './controllers/album.controller';
import { UserAdminController } from './controllers/user-admin.controller';

@Module({ controllers: [AlbumController, UserAdminController] })
export class AppModule {}
