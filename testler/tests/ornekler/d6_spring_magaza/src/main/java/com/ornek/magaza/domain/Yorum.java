package com.ornek.magaza.domain;

import jakarta.persistence.*;

@Entity
public class Yorum {
    @Id
    private Long id;

    @ManyToOne
    private Urun urun;

    private String metin;
}
