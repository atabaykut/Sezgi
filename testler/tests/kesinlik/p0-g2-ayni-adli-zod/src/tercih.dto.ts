import { createZodDto } from 'nestjs-zod';
import { z } from 'zod';

// Aynı adlı, modül-yerel başka bir şema (Immich user-preferences.dto.ts deseni)
const PeopleUpdateSchema = z.object({
  enabled: z.boolean().optional(),
  minimumFaces: z.number().int().min(1).optional(),
});
export class TercihDto extends createZodDto(PeopleUpdateSchema) {}
