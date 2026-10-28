import { Module } from '@nestjs/common';
import { PaylasimController } from './paylasim.controller';
import { VarlikController } from './varlik.controller';
import { KayitController } from './kayit.controller';

@Module({ controllers: [PaylasimController, VarlikController, KayitController] })
export class AppModule {}
