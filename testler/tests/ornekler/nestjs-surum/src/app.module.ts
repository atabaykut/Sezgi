import { Module } from '@nestjs/common';
import { SiparisV1Controller, SiparisV2Controller } from './siparis.controller';

@Module({ controllers: [SiparisV1Controller, SiparisV2Controller] })
export class AppModule {}
