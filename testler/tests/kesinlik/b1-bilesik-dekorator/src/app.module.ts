import { Module } from '@nestjs/common';
import { HesapController, IcController } from './hesap.controller';

@Module({ controllers: [HesapController, IcController] })
export class AppModule {}
