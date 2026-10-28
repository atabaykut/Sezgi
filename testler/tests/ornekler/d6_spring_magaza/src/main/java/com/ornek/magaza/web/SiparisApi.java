package com.ornek.magaza.web;

import com.ornek.magaza.dto.SiparisIstegi;
import jakarta.validation.Valid;
import org.springframework.web.bind.annotation.*;

@RequestMapping("/api/siparisler")
public interface SiparisApi {

    @GetMapping("/{siparisNo}")
    Object getir(@PathVariable("siparisNo") String no);

    @PostMapping
    void ver(@Valid @RequestBody SiparisIstegi istek);

    @DeleteMapping("/{siparisNo}")
    void iptal(@PathVariable("siparisNo") String no);
}
