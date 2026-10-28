package com.ornek.magaza.web;

import com.ornek.magaza.dto.SiparisIstegi;
import com.ornek.magaza.repo.SiparisRepository;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class SiparisController implements SiparisApi {

    private final SiparisRepository siparisler;

    public SiparisController(SiparisRepository siparisler) {
        this.siparisler = siparisler;
    }

    @Override
    public Object getir(String no) {
        return siparisler.findByNo(no);
    }

    @Override
    public void ver(SiparisIstegi istek) {
        siparisler.save(istek.toSiparis());
    }

    @Override
    public void iptal(String no) {
        var siparis = siparisler.findByNo(no);
        if (siparis == null) {
            throw new org.springframework.web.server.ResponseStatusException(org.springframework.http.HttpStatus.NOT_FOUND, "yok");
        }
        siparisler.delete(siparis);
    }
}
