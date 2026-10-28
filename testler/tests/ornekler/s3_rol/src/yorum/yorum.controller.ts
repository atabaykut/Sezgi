import { Body, Controller, Delete, Inject, Post } from '@nestjs/common';
import { YorumServisi } from './yorum.service';
import { z } from 'zod';
import { createZodDto } from 'nestjs-zod';
import { Kategori } from '../sabitler';
import { Roles } from '../roles.decorator';

// Alan tipleri isimli sabitlerle: tip ve kurallar tanımdan okunmalı
const tarihMetni = z.codec(z.iso.datetime({ offset: true }), z.date(), {
  decode: (s) => new Date(s),
  encode: (d) => d.toISOString(),
});
const evetHayir = z.stringbool();
const YorumDurumu = z.enum(['acik', 'gizli']);
const KisaMetin = z.string().max(20);
const YazarSema = z.object({ ad: z.string(), eposta: z.email().optional() });
const KaynakSema = z.object({ url: z.url(), not: z.string().optional() });

const TemelYorumSema = z.object({
  metin: z.string().min(1).max(500),
  puan: z.int().min(1).max(5).optional(),
});
const YorumSema = TemelYorumSema.extend({
  kitapId: z.uuid(),
  tarih: tarihMetni.optional(),
  herkese: evetHayir.optional(),
  durum: YorumDurumu.optional(),
  baslik: KisaMetin.optional(),
  etiketler: z.array(z.uuidv4()).min(1).optional(),
  yazar: YazarSema.optional(),
  kaynaklar: z.array(KaynakSema).optional(),
  ek: z.record(z.string(), z.string()).optional(),
  kategori: z.enum(Kategori).optional(),
}).describe('Yorum');

// Şema başka bir ada atanmış: atama izlenir
const YorumGirdisi = YorumSema;

export class YorumDto extends createZodDto(YorumGirdisi) {}

// Gövdenin kendisi dizi
const TopluSilSema = z.array(z.object({ id: z.uuid(), neden: z.string().optional() }));
export class TopluSilDto extends createZodDto(TopluSilSema) {}

@Controller('yorumlar')
export class YorumController {
  // Alan olarak enjekte edilen servis (constructor dışında)
  @Inject(YorumServisi) private readonly servis!: YorumServisi;

  @Roles('EDITOR')
  @Post()
  yaz(@Body() dto: YorumDto) {
    return this.servis.yaz(dto);
  }

  @Delete('toplu')
  topluSil(@Body() dto: TopluSilDto) {
    return dto;
  }
}
