import { Module } from '@nestjs/common';
import { AssetController } from './controllers/asset.controller';
import { BackupController } from './controllers/backup.controller';

@Module({ controllers: [AssetController, BackupController] })
export class AppModule {}
