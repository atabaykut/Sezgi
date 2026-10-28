package com.ornek.magaza.service;

import com.ornek.magaza.dto.UrunIstegi;
import com.ornek.magaza.dto.UrunYaniti;
import java.util.List;

public interface UrunService {
    List<UrunYaniti> ara(String q, int sayfa);
    UrunYaniti olustur(UrunIstegi istek);
    UrunYaniti guncelle(Long id, UrunIstegi istek);
    void sil(Long id);
}
