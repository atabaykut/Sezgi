import { Module } from '@nestjs/common';
import { APP_GUARD, NestFactory } from '@nestjs/core';
import { AppModule } from '../src/app.module';
import { JwtAuthGuard } from '../src/jwt-auth.guard';

// Yalnızca uçtan uca testlerde kurulan modül: uygulamanın kendisi bu guard'la çalışmaz
@Module({ imports: [AppModule], providers: [{ provide: APP_GUARD, useClass: JwtAuthGuard }] })
export class TestModule {}

export async function testUygulamasi() {
  return NestFactory.create(TestModule);
}
