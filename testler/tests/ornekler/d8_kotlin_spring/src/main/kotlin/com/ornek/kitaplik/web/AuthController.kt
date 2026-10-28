package com.ornek.kitaplik.web

import com.ornek.kitaplik.config.API
import com.ornek.kitaplik.config.ApiAnahtari
import com.ornek.kitaplik.dto.GirisIstegi
import jakarta.validation.Valid
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping("$API/auth")
class AuthController {
    @PostMapping("/giris")
    fun giris(@Valid @RequestBody istek: GirisIstegi): ResponseEntity<*> {
        if (istek.sifre != "dogru-sifre") {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).build<Any>()
        }
        return ResponseEntity.ok(mapOf("token" to "t"))
    }

    /** Anahtarı denetleyen uygulamanın kendi kesiti (AOP) */
    @ApiAnahtari
    @GetMapping("/ben")
    fun ben(): Map<String, String> = mapOf("ad" to "ben")
}
