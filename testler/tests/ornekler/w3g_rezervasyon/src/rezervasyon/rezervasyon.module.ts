import { Module } from '@nestjs/common';
import { RezervasyonController } from './rezervasyon.controller';
import { MisafirController } from './misafir.controller';

@Module({
  controllers: [RezervasyonController, MisafirController],
})
export class RezervasyonModule {}
