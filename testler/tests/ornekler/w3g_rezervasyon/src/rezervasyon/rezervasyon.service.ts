import { Injectable } from '@nestjs/common';
import { PrismaService } from '../prisma.service';

@Injectable()
export class RezervasyonService {
  constructor(private readonly prisma: PrismaService) {}

  listele(varis?: string) {
    return this.prisma.rezervasyon.findMany({ where: { varisTarihi: varis } });
  }

  getir(rezervasyonNo: string) {
    return this.prisma.rezervasyon.findUnique({ where: { rezervasyonNo } });
  }

  guncelle(id: string, veri: object) {
    return this.prisma.rezervasyon.update({ where: { id }, data: veri });
  }

  misafir(id: string) {
    return this.prisma.misafir.findUnique({ where: { id } });
  }
}
