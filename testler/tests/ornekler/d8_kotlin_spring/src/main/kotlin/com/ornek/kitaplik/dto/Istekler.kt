package com.ornek.kitaplik.dto

import com.fasterxml.jackson.annotation.JsonProperty
import jakarta.validation.constraints.*

data class KitapIstegi(
    @field:NotBlank
    @field:Size(min = 2, max = 200)
    val baslik: String,
    @field:Positive
    val sayfaSayisi: Int,
    @JsonProperty("isbn_kodu")
    @field:Pattern(regexp = "\\d{13}")
    val isbn: String?,
    val yazarAdi: String,
    val etiketler: List<String> = emptyList(),
    val tur: Tur? = null,
)

/** Kurucu özelliğindeki hedefsiz kısıt Kotlin'de parametreye gider */
data class KitapGuncelle(
    @NotBlank
    val baslik: String?,
    val ozet: String? = null,
)

enum class Tur { ROMAN, SIIR }

data class KitapYaniti(val id: Long, val baslik: String, val yazar: String?)

data class GirisIstegi(
    @field:Email
    val eposta: String,
    @field:Size(min = 8)
    val sifre: String,
)
