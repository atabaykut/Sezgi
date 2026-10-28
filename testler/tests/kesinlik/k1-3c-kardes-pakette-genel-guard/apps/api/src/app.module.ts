import { Module } from '@nestjs/common';
import { AController } from './a.controller';

// Kardeş paketin modülü içe aktarılmıyor: onun APP_GUARD'ı bu uygulamada çalışmaz
@Module({ controllers: [AController] })
export class AppModule {}
