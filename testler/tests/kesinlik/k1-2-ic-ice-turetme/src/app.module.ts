import { Module } from '@nestjs/common';
import { TController } from './t.controller';

@Module({ controllers: [TController] })
export class AppModule {}
