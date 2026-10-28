import { Module } from '@nestjs/common';
import { AController } from './a.controller';

@Module({ controllers: [AController] })
export class AppModule {}
