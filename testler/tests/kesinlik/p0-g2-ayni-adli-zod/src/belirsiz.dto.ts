import { createZodDto } from 'nestjs-zod';

// OrtakSchema bu dosyada tanımlı değil, içe aktarılmamış; iki dosyada aynı adla var: hangisi olduğu kesin değil
export class BelirsizDto extends createZodDto(OrtakSchema) {}
