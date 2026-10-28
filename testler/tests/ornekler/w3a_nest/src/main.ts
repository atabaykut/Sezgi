import { NestFactory } from '@nestjs/core';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import { AppModule } from './app.module';

async function bootstrap() {
  const app = await NestFactory.create(AppModule);
  app.setGlobalPrefix('api');
  const config = new DocumentBuilder().build();
  const options = {
    operationIdFactory: (controllerKey: string, methodKey: string) => methodKey,
  };
  SwaggerModule.createDocument(app, config, options);
  await app.listen(3000);
}
bootstrap();
