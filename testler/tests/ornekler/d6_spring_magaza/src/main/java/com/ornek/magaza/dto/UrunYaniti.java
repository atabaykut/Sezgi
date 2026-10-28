package com.ornek.magaza.dto;

import com.ornek.magaza.domain.Urun;
import java.math.BigDecimal;

public record UrunYaniti(Long id, String ad, BigDecimal fiyat) {
    public static UrunYaniti from(Urun u) {
        return new UrunYaniti(u.getId(), u.getAd(), u.getFiyat());
    }
}
