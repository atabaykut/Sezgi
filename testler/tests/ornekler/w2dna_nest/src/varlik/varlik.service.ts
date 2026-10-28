import { ConflictException, ForbiddenException, HttpException, HttpStatus, NotFoundException } from '@nestjs/common';
import { TemelService } from '../ortak/temel.service';
import { PrismaService } from '../prisma.service';

export class VarlikService extends TemelService {
  constructor(private readonly prisma: PrismaService) {
    super();
  }

  async topluSil(kullaniciId: string, ids: string[], force?: boolean) {
    await this.erisimIste(ids);
    await this.prisma.varlik.updateMany({
      where: { id: { in: ids } },
      data: { deletedAt: new Date(), durum: force ? VarlikDurumu.Silindi : VarlikDurumu.Copte },
    });
  }

  async sil(kullaniciId: string, id: string) {
    const varlik = await this.prisma.varlik.findUnique({ where: { id } });
    if (!varlik) {
      throw new NotFoundException('Varlik yok');
    }
    if (varlik.sahipId !== kullaniciId) {
      throw new ForbiddenException('Yetki yok');
    }
    if (varlik.kilitli) {
      throw new HttpException('Kilitli', HttpStatus.CONFLICT);
    }
    await this.prisma.varlik.delete({ where: { id } });
  }

  async adlandir(id: string, ad: string) {
    const ayni = await this.prisma.varlik.findFirst({ where: { ad } });
    if (ayni) {
      throw new ConflictException('Ad kullanimda');
    }
    return this.prisma.varlik.update({ where: { id }, data: { ad } });
  }
}
