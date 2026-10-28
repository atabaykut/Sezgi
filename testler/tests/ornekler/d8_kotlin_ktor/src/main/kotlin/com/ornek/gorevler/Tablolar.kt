package com.ornek.gorevler

import org.jetbrains.exposed.dao.id.LongIdTable
import org.jetbrains.exposed.sql.ReferenceOption
import org.jetbrains.exposed.sql.Table

object Kullanicilar : LongIdTable("kullanicilar") {
    val ad = varchar("ad", 100)
}

object Gorevler : LongIdTable("gorevler") {
    val baslik = varchar("baslik", 200)
    val tamamlandi = bool("tamamlandi").default(false)
    val sahip = reference("sahip_id", Kullanicilar, onDelete = ReferenceOption.CASCADE)
}

object EtiketTable : Table() {
    val gorev = reference("gorev_id", Gorevler, onDelete = ReferenceOption.CASCADE)
    val ad = varchar("ad", 50)
}
