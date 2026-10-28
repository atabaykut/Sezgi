package ornek.web

import io.ktor.routing.Routing
import io.ktor.routing.delete
import io.ktor.routing.get
import io.ktor.routing.post
import io.ktor.routing.route

fun Routing.profiller(profilDenetleyici: ProfilDenetleyici) {
    route("profiles/{username}") {
        get { profilDenetleyici.getir(this.context) }
        route("follow") {
            post { profilDenetleyici.takipEt(this.context) }
            delete { profilDenetleyici.birak(this.context) }
        }
    }
}
