package com.ornek.magaza.dto;

import jakarta.validation.constraints.NotBlank;

public abstract class TemelIstek {
    @NotBlank
    private String kanal;
}
