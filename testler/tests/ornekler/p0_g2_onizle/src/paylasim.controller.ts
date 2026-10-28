import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';
import { createZodDto } from 'nestjs-zod';
import { z } from 'zod';

// Immich shared-link deseni: albumId alan düzeyinde isteğe bağlı, ama type=ALBUM iken nesne düzeyindeki kural ister
const PaylasimSchema = z
  .object({
    type: z.enum(['ALBUM', 'INDIVIDUAL']),
    albumId: z.string().uuid().optional(),
    assetIds: z.array(z.string().uuid()).optional(),
  })
  .superRefine(({ type, albumId }, ctx) => {
    if (type === 'ALBUM' && !albumId) {
      ctx.addIssue({ code: 'custom', message: 'albumId is required for type ALBUM' });
    }
  });
export class PaylasimDto extends createZodDto(PaylasimSchema) {}

@Controller('paylasimlar')
@UseGuards(AuthGuard('jwt'))
export class PaylasimController {
  @Post()
  olustur(@Body() dto: PaylasimDto) {}
}
