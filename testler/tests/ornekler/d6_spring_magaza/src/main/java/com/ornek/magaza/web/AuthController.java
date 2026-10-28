package com.ornek.magaza.web;

import com.ornek.magaza.dto.GirisIstegi;
import jakarta.validation.Valid;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.multipart.MultipartFile;

@RestController
@RequestMapping("/api/auth")
public class AuthController {

    @PostMapping("/giris")
    public ResponseEntity<?> giris(@Valid @RequestBody GirisIstegi istek) {
        if (!istek.sifre().equals("x")) {
            return ResponseEntity.status(401).build();
        }
        return ResponseEntity.ok().build();
    }

    @PostMapping(value = "/avatar", consumes = "multipart/form-data")
    public void avatar(@RequestParam("dosya") MultipartFile dosya, @RequestHeader("X-Istek") String istek) {
    }
}
