package com.ornek.magaza.domain;

import jakarta.persistence.*;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;

@Entity
@Table(name = "urunler")
public class Urun {
    @Id
    @GeneratedValue
    private Long id;

    @Column(name = "urun_adi")
    private String ad;

    private BigDecimal fiyat;

    private Integer stok;

    @OneToMany(mappedBy = "urun", cascade = CascadeType.ALL, orphanRemoval = true)
    private List<Yorum> yorumlar = new ArrayList<>();

    @Transient
    private String gecici;

    public Long getId() { return id; }
    public String getAd() { return ad; }
    public BigDecimal getFiyat() { return fiyat; }
}
