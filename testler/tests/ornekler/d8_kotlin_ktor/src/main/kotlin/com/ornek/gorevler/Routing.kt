package com.ornek.gorevler

import io.ktor.http.*
import io.ktor.server.application.*
import io.ktor.server.auth.*
import io.ktor.server.plugins.*
import io.ktor.server.request.*
import io.ktor.server.response.*
import io.ktor.server.routing.*
import io.ktor.server.util.*

const val API = "/api/v1"

fun Application.configureRouting() {
    val servis = GorevServisi()
    val gelistirme = environment.developmentMode
    routing {
        get("/saglik") {
            call.respondText("ok")
        }
        route(API) {
            gorevRotalari(servis)
            authenticate("auth-jwt") {
                route("/yonetim") {
                    rolIle("ADMIN") {
                        delete("/gorevler/{id}") {
                            val id = call.parameters["id"]!!.toLong()
                            servis.sil(id)
                            call.respond(HttpStatusCode.NoContent)
                        }
                    }
                }
            }
        }
        if (gelistirme) {
            get("/hata-ayikla") {
                call.respondText("hata ayıklama")
            }
        }
    }
}

fun Route.gorevRotalari(
    servis: GorevServisi,
    yorumlar: YorumDenetleyici = YorumDenetleyici(),
    bildirim: BildirimIstemcisi = BildirimIstemcisi(),
) {
    route("/gorevler") {
        get {
            val sayfa = call.request.queryParameters["sayfa"]?.toIntOrNull() ?: 1
            call.respond(servis.listele(sayfa))
        }
        get("/{id}") {
            val id = call.parameters["id"]?.toLongOrNull() ?: return@get call.respond(HttpStatusCode.BadRequest)
            val gorev = servis.getir(id) ?: throw NotFoundException()
            call.respond(gorev)
        }
        authenticate("auth-jwt") {
            post {
                val istek = call.receive<GorevIstegi>()
                val yeni = servis.olustur(istek)
                bildirim.bildir(yeni.id)
                bildirim.dizinle(yeni.id)
                call.respond(HttpStatusCode.Created, yeni)
            }
            put("/{id}") {
                val id = call.parameters.getOrFail<Long>("id")
                val istek: GorevGuncelle = call.receive()
                call.respond(servis.guncelle(id, istek))
            }
            post("/{id}/yorumlar") {
                yorumlar.ekle(this.call)
            }
        }
        authenticate("auth-jwt", optional = true) {
            get("/ozet") {
                call.respond(GorevOzeti(0))
            }
        }
    }
}

/** Yalnızca verilen roldeki kullanıcılar (rol denetimi uygulamanın kendi eklentisinde) */
fun Route.rolIle(rol: String, build: Route.() -> Unit) {
    install(RolYetkisi) {
        this.rol = rol
    }
    build()
}
