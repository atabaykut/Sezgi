package com.ornek.gorevler

import io.ktor.http.*
import io.ktor.server.application.*
import io.ktor.server.request.*
import io.ktor.server.response.*

/** İşleyici isteği denetleyiciye verir: girdiler ve yanıt burada okunur */
class YorumDenetleyici {
    suspend fun ekle(ctx: ApplicationCall) {
        val gorevId = ctx.parameters["id"]?.toLong()
        val istek = ctx.receive<YorumIstegi>()
        ctx.respond(HttpStatusCode.Created, YorumYaniti(gorevId ?: 0, istek.metin))
    }
}
