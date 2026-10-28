import { Module } from '@nestjs/common';
import { APP_GUARD } from '@nestjs/core';
import { TransferModule } from './transfer/transfer.module';
import { JwtAuthGuard } from './jwt-auth.guard';

@Module({
  imports: [TransferModule],
  providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }],
})
export class AppModule {}
