package com.ornek.magaza.web;

import com.ornek.magaza.config.ApiYollari;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class SaglikController {
    @GetMapping(ApiYollari.SAGLIK)
    public String saglik() {
        return "UP";
    }

    @GetMapping("/api/rapor/gunluk")
    public String gunluk() {
        return "rapor";
    }
}
