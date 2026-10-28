import { Module } from '@nestjs/common';
import { KisiController } from './kisi.controller';

@Module({ controllers: [KisiController] })
export class AppModule {}
