package com.ornek.gorevler

import org.jetbrains.exposed.sql.*
import org.jetbrains.exposed.sql.SqlExpressionBuilder.eq
import org.jetbrains.exposed.sql.transactions.transaction

class GorevServisi {
    fun listele(sayfa: Int): List<GorevYaniti> = transaction {
        Gorevler.selectAll().limit(20).map { GorevYaniti(it[Gorevler.id].value, it[Gorevler.baslik], it[Gorevler.tamamlandi]) }
    }

    fun getir(id: Long): GorevYaniti? = transaction {
        Gorevler.selectAll().where { Gorevler.id eq id }.singleOrNull()?.let { GorevYaniti(id, it[Gorevler.baslik], it[Gorevler.tamamlandi]) }
    }

    fun olustur(istek: GorevIstegi): GorevYaniti = transaction {
        val id = Gorevler.insertAndGetId {
            it[baslik] = istek.baslik
        }
        GorevYaniti(id.value, istek.baslik, false)
    }

    fun guncelle(id: Long, g: GorevGuncelle): GorevYaniti {
        if (g.baslik == "") {
            throw YetkiYok()
        }
        transaction {
            Gorevler.update({ Gorevler.id eq id }) {
                it[tamamlandi] = g.tamamlandi
            }
        }
        return GorevYaniti(id, g.baslik ?: "", g.tamamlandi)
    }

    fun sil(id: Long) = transaction {
        Gorevler.deleteWhere { Gorevler.id eq id }
    }
}
