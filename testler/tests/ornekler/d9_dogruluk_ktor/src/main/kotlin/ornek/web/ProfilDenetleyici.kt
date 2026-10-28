package ornek.web

import io.ktor.application.ApplicationCall
import io.ktor.response.respond
import ornek.alan.KullaniciServisi

class ProfilDenetleyici(private val kullaniciServisi: KullaniciServisi) {
    suspend fun getir(ctx: ApplicationCall) {
        val ad = ctx.parameters["username"]!!
        ctx.respond(kullaniciServisi.profil("ben@ornek.com", ad))
    }

    suspend fun takipEt(ctx: ApplicationCall) {
        val ad = ctx.parameters["username"]!!
        ctx.respond(kullaniciServisi.takipEt("ben@ornek.com", ad))
    }

    suspend fun birak(ctx: ApplicationCall) {
        val ad = ctx.parameters["username"]!!
        ctx.respond(kullaniciServisi.birak("ben@ornek.com", ad))
    }
}
