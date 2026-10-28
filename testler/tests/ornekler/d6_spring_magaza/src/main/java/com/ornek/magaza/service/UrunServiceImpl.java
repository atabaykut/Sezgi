package com.ornek.magaza.service;

import com.ornek.magaza.domain.Urun;
import com.ornek.magaza.dto.UrunIstegi;
import com.ornek.magaza.dto.UrunYaniti;
import com.ornek.magaza.event.UrunOlusturuldu;
import com.ornek.magaza.exception.StokYokException;
import com.ornek.magaza.repo.UrunRepository;
import java.util.List;
import org.springframework.context.ApplicationEventPublisher;
import org.springframework.stereotype.Service;

@Service
public class UrunServiceImpl implements UrunService {
    private final UrunRepository urunRepository;
    private final ApplicationEventPublisher yayinci;
    private final BildirimServisi bildirim;

    public UrunServiceImpl(UrunRepository urunRepository, ApplicationEventPublisher yayinci, BildirimServisi bildirim) {
        this.urunRepository = urunRepository;
        this.yayinci = yayinci;
        this.bildirim = bildirim;
    }

    @Override
    public List<UrunYaniti> ara(String q, int sayfa) {
        return urunRepository.findByAdContaining(q).stream().map(UrunYaniti::from).toList();
    }

    @Override
    public UrunYaniti olustur(UrunIstegi istek) {
        Urun u = urunRepository.save(new Urun());
        yayinci.publishEvent(new UrunOlusturuldu(u.getId()));
        bildirim.gonder("yeni urun");
        return UrunYaniti.from(u);
    }

    @Override
    public UrunYaniti guncelle(Long id, UrunIstegi istek) {
        Urun u = urunRepository.findById(id).orElseThrow();
        if (u.getFiyat() == null) {
            throw new StokYokException("stok yok");
        }
        return UrunYaniti.from(urunRepository.save(u));
    }

    @Override
    public void sil(Long id) {
        urunRepository.deleteById(id);
    }
}
