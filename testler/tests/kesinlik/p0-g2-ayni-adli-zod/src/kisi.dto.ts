import { createZodDto } from 'nestjs-zod';
import { z } from 'zod';
import { Durum } from './durum';

const PeopleUpdateSchema = z.object({
  people: z.array(z.string()),
});
export class KisilerDto extends createZodDto(PeopleUpdateSchema) {}

const DurumSchema = z.object({
  durum: z.enum(Durum),
});
export class DurumDto extends createZodDto(DurumSchema) {}
