import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { AController } from './a.controller';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({
  controllers: [AController],
  providers: [
    // { provide: APP_GUARD, useClass: JwtAuthGuard },
    /* { provide: APP_GUARD, useClass: JwtAuthGuard } */
  ],
})
export class AppModule {}
// Eski kurulum: providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }]
