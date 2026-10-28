import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { RezervasyonController } from './rezervasyon/rezervasyon.controller';
import { MisafirController } from './rezervasyon/misafir.controller';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({
  controllers: [RezervasyonController, MisafirController],
  providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }],
})
export class AppModule {}
