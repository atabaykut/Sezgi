package com.ornek.gorevler

import io.ktor.http.*
import io.ktor.serialization.kotlinx.json.*
import io.ktor.server.application.*
import io.ktor.server.auth.*
import io.ktor.server.auth.jwt.*
import io.ktor.server.plugins.contentnegotiation.*
import io.ktor.server.plugins.requestvalidation.*
import io.ktor.server.plugins.statuspages.*
import io.ktor.server.response.*

fun main(args: Array<String>) = io.ktor.server.netty.EngineMain.main(args)

fun Application.module() {
    install(ContentNegotiation) {
        json()
    }
    install(Authentication) {
        jwt("auth-jwt") {
            realm = "gorevler"
        }
    }
    install(StatusPages) {
        exception<YetkiYok> { call, cause ->
            call.respond(HttpStatusCode.Forbidden, cause.message ?: "")
        }
    }
    install(RequestValidation) {
        validate<GorevIstegi> { g ->
            if (g.baslik.isBlank()) ValidationResult.Invalid("boş başlık") else ValidationResult.Valid
        }
    }
    configureRouting()
}
