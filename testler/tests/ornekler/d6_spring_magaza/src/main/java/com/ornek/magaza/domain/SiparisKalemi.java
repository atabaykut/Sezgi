package com.ornek.magaza.domain;

import jakarta.persistence.*;
import org.hibernate.annotations.OnDelete;
import org.hibernate.annotations.OnDeleteAction;

@Entity
public class SiparisKalemi {
    @Id
    private Long id;

    @ManyToOne
    @JoinColumn(name = "siparis_ref")
    @OnDelete(action = OnDeleteAction.CASCADE)
    private Siparis siparis;
}
