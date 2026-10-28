package com.ornek.kitaplik.service

import com.ornek.kitaplik.domain.Kitap
import com.ornek.kitaplik.dto.KitapIstegi
import com.ornek.kitaplik.dto.KitapYaniti
import com.ornek.kitaplik.repo.KitapDeposu
import com.ornek.kitaplik.repo.YazarDeposu
import org.springframework.context.ApplicationEventPublisher
import org.springframework.context.event.EventListener
import org.springframework.data.repository.findByIdOrNull
import org.springframework.http.HttpStatus
import org.springframework.stereotype.Component
import org.springframework.stereotype.Service
import org.springframework.transaction.annotation.Transactional
import org.springframework.web.bind.annotation.ResponseStatus
import org.springframework.web.client.RestTemplate
import org.springframework.web.server.ResponseStatusException

@Service
class KitapServisi(
    private val kitapDeposu: KitapDeposu,
    private val yazarDeposu: YazarDeposu,
    private val yayinci: ApplicationEventPublisher,
    private val restTemplate: RestTemplate,
) {
    fun listele(q: String?): List<KitapYaniti> = kitapDeposu.findAll().map { KitapYaniti(it.id!!, it.baslik, it.yazar.ad) }

    fun getir(id: Long): KitapYaniti {
        val kitap = kitapDeposu.findByIdOrNull(id) ?: throw KitapBulunamadi(id)
        return KitapYaniti(kitap.id!!, kitap.baslik, null)
    }

    @Transactional
    fun olustur(istek: KitapIstegi): KitapYaniti {
        val yazar = yazarDeposu.findByAd(istek.yazarAdi) ?: throw ResponseStatusException(HttpStatus.BAD_REQUEST, "yazar yok")
        val kitap = kitapDeposu.save(Kitap(baslik = istek.baslik, yazar = yazar))
        yayinci.publishEvent(KitapEklendi(kitap.id!!))
        restTemplate.postForObject("https://arama.ornek.com/dizin", kitap.baslik, String::class.java)
        return KitapYaniti(kitap.id!!, kitap.baslik, yazar.ad)
    }

    fun sil(id: Long) {
        kitapDeposu.deleteById(id)
    }
}

@ResponseStatus(HttpStatus.NOT_FOUND)
class KitapBulunamadi(id: Long) : RuntimeException("kitap $id yok")

data class KitapEklendi(val id: Long)

@Component
class KitapDinleyici {
    @EventListener
    fun kitapEklendi(olay: KitapEklendi) {
        println(olay.id)
    }
}
