import { Module } from '@nestjs/common';
import { TransferController } from './transfer.controller';

@Module({
  controllers: [TransferController],
})
export class TransferModule {}
