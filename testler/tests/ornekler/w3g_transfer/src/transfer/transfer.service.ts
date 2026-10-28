import { Injectable } from '@nestjs/common';
import { PrismaService } from '../prisma.service';

@Injectable()
export class TransferService {
  constructor(private readonly prisma: PrismaService) {}

  listele(rezervasyonRef?: string) {
    return this.prisma.transfer.findMany({ where: { rezervasyonRef } });
  }

  async olustur(veri: { rezervasyonRef: string; yolcuId: string; aracId: string; yolcuSayisi: number }) {
    // Rezervasyon uygulamasına, transferin taşıdığı rezervasyon numarasıyla sorulur
    const rezervasyon = await fetch(`${process.env.REZERVASYON_URL}/api/rezervasyonlar/${veri.rezervasyonRef}`);
    if (!rezervasyon.ok) {
      throw new Error('rezervasyon yok');
    }
    return this.prisma.transfer.create({ data: { id: 't1', ...veri } });
  }

  guncelle(id: string, veri: object) {
    return this.prisma.transfer.update({ where: { id }, data: veri });
  }
}
