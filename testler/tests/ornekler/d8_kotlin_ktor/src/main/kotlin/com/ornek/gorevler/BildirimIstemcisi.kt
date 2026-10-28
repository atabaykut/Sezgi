package com.ornek.gorevler

import io.ktor.client.*
import io.ktor.client.engine.cio.*
import io.ktor.client.plugins.*
import io.ktor.client.request.*

/** Görev olaylarını bildirim hizmetine iletir (giden HTTP) */
class BildirimIstemcisi {
    private val client = HttpClient(CIO)
    private val aramaClient = HttpClient(CIO) {
        defaultRequest {
            url("https://arama.ornek.com/")
        }
    }

    suspend fun bildir(id: Long) {
        client.post("https://bildirim.ornek.com/v1/olaylar") {
            setBody(id.toString())
        }
    }

    /** Göreli adres: taban `defaultRequest`'ten */
    suspend fun dizinle(id: Long) {
        aramaClient.put("/v2/dizin/$id")
    }
}
