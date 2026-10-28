package com.ornek.kitaplik.domain

import jakarta.persistence.*
import org.hibernate.annotations.SQLDelete

@Entity
@Table(name = "kitaplar")
class Kitap(
    @Id
    @GeneratedValue
    var id: Long? = null,
    @Column(name = "kitap_basligi")
    var baslik: String,
    @ManyToOne
    var yazar: Yazar,
    @OneToMany(mappedBy = "kitap", cascade = [CascadeType.ALL], orphanRemoval = true)
    var yorumlar: MutableList<Yorum> = mutableListOf(),
)

@Entity
class Yorum(
    @Id
    var id: Long? = null,
    @ManyToOne
    var kitap: Kitap,
    var metin: String,
)

@Entity
@SQLDelete(sql = "update yazar set silindi = true where id = ?")
class Yazar(
    @Id
    var id: Long? = null,
    var ad: String,
)
