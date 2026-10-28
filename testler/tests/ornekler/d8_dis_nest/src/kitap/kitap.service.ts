import { Injectable } from '@nestjs/common';
import { TemelServis } from '../temel.service';

@Injectable()
export class KitapService extends TemelServis {
  listele() {
    return this.prisma.kitap.findMany();
  }

  async olustur(veri: { baslik: string }) {
    return this.prisma.$transaction(async (tx) => {
      const kitap = await tx.kitap.create({ data: { id: '1', baslik: veri.baslik } });
      await tx.etiket.upsert({ where: { id: 'yeni' }, create: { id: 'yeni' }, update: {} });
      return kitap;
    });
  }

  guncelle(id: string, veri: object) {
    return this.yaz(id, veri);
  }

  private yaz(id: string, veri: object) {
    return this.prisma.kitap.update({ where: { id }, data: veri });
  }

  async sil(id: string) {
    await this.prisma.kitap.delete({ where: { id } });
    await this.kaydet('silindi');
  }
}
