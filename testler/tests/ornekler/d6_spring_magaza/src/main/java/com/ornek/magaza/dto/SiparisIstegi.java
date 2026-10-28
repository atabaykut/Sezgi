package com.ornek.magaza.dto;

import com.ornek.magaza.domain.Siparis;
import jakarta.validation.constraints.NotNull;

public class SiparisIstegi {
    @NotNull
    private Long urunId;

    public Siparis toSiparis() {
        return new Siparis();
    }
}
