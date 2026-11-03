import { PrismaService } from './prisma.service';

export abstract class TemelServis {
  constructor(protected readonly prisma: PrismaService) {}

  // Üst sınıftan miras alınan yardımcı: alt sınıflar this.kaydet() ile çağırır
  protected async kaydet(olay: string) {
    await this.prisma.gunluk.create({ data: { id: olay } });
  }
}
