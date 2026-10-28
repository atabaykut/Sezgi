package com.ornek.magaza.web;

import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping({"/api/yonetim"})
@PreAuthorize("hasAnyRole('ADMIN', 'EDITOR')")
public class YonetimController {

    @GetMapping("/rapor")
    public String rapor() {
        return "ok";
    }

    @PostMapping("/onbellek/temizle")
    @PreAuthorize("@yetki.yonetici(authentication)")
    public void temizle() {
    }
}
