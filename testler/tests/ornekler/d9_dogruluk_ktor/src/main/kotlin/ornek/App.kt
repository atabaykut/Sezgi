package ornek

import io.ktor.application.Application
import io.ktor.application.install
import io.ktor.routing.Routing
import io.ktor.server.engine.embeddedServer
import io.ktor.server.netty.Netty
import ornek.alan.KullaniciDeposu
import ornek.alan.KullaniciServisi
import ornek.web.ProfilDenetleyici
import ornek.web.profiller

fun main() {
    embeddedServer(Netty, 8080, module = Application::modul).start(wait = true)
}

fun Application.modul() {
    val denetleyici = ProfilDenetleyici(KullaniciServisi(KullaniciDeposu()))
    install(Routing) {
        profiller(denetleyici)
    }
}
