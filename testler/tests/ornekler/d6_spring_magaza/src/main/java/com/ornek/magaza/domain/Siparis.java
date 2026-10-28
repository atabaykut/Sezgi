package com.ornek.magaza.domain;

import jakarta.persistence.*;
import java.util.List;
import org.hibernate.annotations.SQLDelete;

@Entity
@SQLDelete(sql = "UPDATE siparis SET silindi = true WHERE id = ?")
public class Siparis {
    @Id
    private Long id;

    private String no;

    @OneToMany(mappedBy = "siparis", cascade = {CascadeType.PERSIST, CascadeType.REMOVE})
    private List<SiparisKalemi> kalemler;
}
