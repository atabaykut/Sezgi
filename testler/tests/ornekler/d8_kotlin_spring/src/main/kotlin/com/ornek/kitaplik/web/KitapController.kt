package com.ornek.kitaplik.web

import com.ornek.kitaplik.config.ApiYollari
import com.ornek.kitaplik.dto.KitapGuncelle
import com.ornek.kitaplik.dto.KitapIstegi
import com.ornek.kitaplik.dto.KitapYaniti
import com.ornek.kitaplik.service.KitapServisi
import jakarta.validation.Valid
import org.springframework.http.HttpStatus
import org.springframework.security.access.prepost.PreAuthorize
import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping(ApiYollari.KITAPLAR)
class KitapController(private val kitapServisi: KitapServisi) {

    /** Kitapları başlığa göre arar. */
    @GetMapping
    fun listele(
        @RequestParam(required = false) q: String?,
        @RequestParam sayfa: Int = 0,
        @RequestParam adet: Int,
    ): List<KitapYaniti> = kitapServisi.listele(q)

    @GetMapping("/{id}")
    fun getir(@PathVariable id: Long): KitapYaniti = kitapServisi.getir(id)

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    fun olustur(@Valid @RequestBody istek: KitapIstegi): KitapYaniti {
        return kitapServisi.olustur(istek)
    }

    @PutMapping("/{id}")
    fun guncelle(@PathVariable id: Long, @Valid @RequestBody istek: KitapGuncelle): KitapYaniti = kitapServisi.getir(id)

    @DeleteMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    fun sil(@PathVariable("id") kitapId: Long) {
        kitapServisi.sil(kitapId)
    }
}
