package ornek.alan

import org.jetbrains.exposed.dao.LongIdTable
import org.jetbrains.exposed.sql.Column
import org.jetbrains.exposed.sql.JoinType
import org.jetbrains.exposed.sql.Table
import org.jetbrains.exposed.sql.and
import org.jetbrains.exposed.sql.deleteWhere
import org.jetbrains.exposed.sql.insert
import org.jetbrains.exposed.sql.select
import org.jetbrains.exposed.sql.transactions.transaction

internal object Users : LongIdTable() {
    val email: Column<String> = varchar("email", 200)
    val username: Column<String> = varchar("username", 100)
}

internal object Follows : Table() {
    val user: Column<Long> = long("user").primaryKey()
    val follower: Column<Long> = long("user_follower").primaryKey()
}

class KullaniciDeposu {
    fun adaGore(ad: String): Long? {
        return transaction {
            Users.select { Users.username eq ad }.map { it[Users.id].value }.firstOrNull()
        }
    }

    fun takipEdiyorMu(eposta: String, kimlik: Long): Boolean {
        return transaction {
            Users.join(Follows, JoinType.INNER, additionalConstraint = { Follows.user eq Users.id and (Follows.follower eq kimlik) })
                .select { Users.email eq eposta }
                .count() > 0
        }
    }

    fun takipEt(eposta: String, ad: String): Long {
        val hedef = adaGore(ad) ?: throw IllegalStateException("yok")
        transaction {
            Follows.insert { row ->
                row[Follows.user] = hedef
                row[follower] = 1L
            }
        }
        return hedef
    }

    fun birak(eposta: String, ad: String): Long {
        val hedef = adaGore(ad) ?: throw IllegalStateException("yok")
        transaction {
            Follows.deleteWhere { Follows.user eq hedef }
        }
        return hedef
    }
}
