package com.ornek.gorevler

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class GorevIstegi(
    val baslik: String,
    val aciklama: String? = null,
    val oncelik: Int = 1,
    val etiketler: List<String>,
)

@Serializable
data class GorevGuncelle(
    val baslik: String?,
    @SerialName("bitti")
    val tamamlandi: Boolean,
)

@Serializable
data class GorevYaniti(val id: Long, val baslik: String, val tamamlandi: Boolean)

@Serializable
data class GorevOzeti(val toplam: Int)

@Serializable
data class YorumIstegi(val metin: String)

@Serializable
data class YorumYaniti(val gorevId: Long, val metin: String)

class YetkiYok : RuntimeException("yetki yok")
