package com.ornek.notlar.domain;

import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.Table;

@Entity
@Table(name = "notlar")
public class Note {
    @Id
    private Long id;

    private Long ownerId;

    public Long getOwnerId() {
        return ownerId;
    }
}
