import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { KitapController } from './kitap/kitap.controller';
import { YorumController } from './yorum/yorum.controller';
import { JwtAuthGuard } from './jwt-auth.guard';

export const kontrolculer = [KitapController];

@Module({
  controllers: [...kontrolculer, YorumController],
  providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }],
})
export class AppModule {}
