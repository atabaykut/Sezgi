import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { PController } from './p.controller';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({ controllers: [PController], providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }] })
export class AppModule {}
