import { Module } from '@nestjs/common';
import { VarlikController } from './varlik/varlik.controller';
import { CopController } from './varlik/cop.controller';

@Module({ controllers: [VarlikController, CopController] })
export class AppModule {}
