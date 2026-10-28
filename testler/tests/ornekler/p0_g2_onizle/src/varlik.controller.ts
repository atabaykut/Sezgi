import { Body, Controller, ParseFilePipe, Post, UploadedFile, UploadedFiles, UseGuards, UseInterceptors } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { FileFieldsInterceptor, FileInterceptor } from '@nestjs/platform-express';
import { IsString } from 'class-validator';

export class VarlikDto {
  @IsString()
  deviceId!: string;
}

// Immich asset upload deseni: dosya gövdenin parçası değil, ayrı bir parametre ve dosya doğrulayıcısı ile gelir
@Controller('varliklar')
@UseGuards(AuthGuard('jwt'))
export class VarlikController {
  @Post()
  @UseInterceptors(FileFieldsInterceptor([{ name: 'assetData', maxCount: 1 }]))
  yukle(@UploadedFiles(new ParseFilePipe({ validators: [] })) dosyalar: Record<string, unknown>, @Body() dto: VarlikDto) {}

  @Post('tek')
  @UseInterceptors(FileInterceptor('dosya'))
  tek(@UploadedFile() dosya: unknown) {}
}
