import { Module } from '@nestjs/common';
import { SiparisController } from './siparis.controller';
import { YonetimController } from './yonetim';

@Module({ controllers: [SiparisController, YonetimController] })
export class AppModule {}
