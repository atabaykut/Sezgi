import { Injectable } from '@nestjs/common';
import { PrismaService } from '../prisma.service';
import { KitapService } from '../kitap/kitap.service';

@Injectable()
export class YorumServisi {
  constructor(private readonly prisma: PrismaService, private readonly kitap: KitapService) {}

  async yaz(veri: { kitapId: string }) {
    // this.kitap bir servis alanıdır, Prisma modeli değil: "Kitap" tablosuna bağ sayılmamalı
    const sayi = this.kitap.count();
    await this.prisma.kitap.findUnique({ where: { id: veri.kitapId } });
    const yorum = await this.prisma.yorum.create({ data: { id: String(sayi), kitapId: veri.kitapId } });
    // Dış sisteme bildirim: uygulamanın dışına yazar, geri alınamaz
    await fetch('https://bildirim.example.com/v1/yorum', { method: 'POST', body: JSON.stringify({ kitapId: veri.kitapId }) });
    return yorum;
  }
}
