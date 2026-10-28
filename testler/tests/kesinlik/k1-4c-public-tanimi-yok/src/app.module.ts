import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { JwtAuthGuard } from '@acme/kimlik';
import { AController } from './a.controller';

@Module({ controllers: [AController], providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }] })
export class AppModule {}
