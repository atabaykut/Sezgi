package com.ornek.kitaplik.web

import com.ornek.kitaplik.repo.YazarDeposu
import org.springframework.security.access.prepost.PreAuthorize
import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping("/api/yonetim")
@PreAuthorize("hasAnyRole('ADMIN', 'EDITOR')")
class YonetimController(private val yazarDeposu: YazarDeposu) {
    @GetMapping("/rapor")
    fun rapor(): Map<String, Any> = mapOf("yazar" to yazarDeposu.count())

    @DeleteMapping("/yazarlar/{id}")
    fun yazarSil(@PathVariable id: Long) = yazarDeposu.deleteById(id)
}
