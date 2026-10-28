import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { TransferController } from './transfer/transfer.controller';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({
  controllers: [TransferController],
  providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }],
})
export class AppModule {}
