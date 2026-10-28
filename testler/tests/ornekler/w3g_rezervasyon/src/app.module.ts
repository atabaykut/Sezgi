import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { RezervasyonModule } from './rezervasyon/rezervasyon.module';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({
  imports: [RezervasyonModule],
  providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }],
})
export class AppModule {}
