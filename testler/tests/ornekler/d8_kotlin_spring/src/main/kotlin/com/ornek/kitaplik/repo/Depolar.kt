package com.ornek.kitaplik.repo

import com.ornek.kitaplik.domain.Kitap
import com.ornek.kitaplik.domain.Yazar
import org.springframework.data.jpa.repository.JpaRepository

interface KitapDeposu : JpaRepository<Kitap, Long>

interface YazarDeposu : JpaRepository<Yazar, Long> {
    fun findByAd(ad: String): Yazar?
}
