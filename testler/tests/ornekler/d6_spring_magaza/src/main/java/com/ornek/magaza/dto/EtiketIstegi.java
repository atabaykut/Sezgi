package com.ornek.magaza.dto;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;

public record EtiketIstegi(@NotBlank @Size(max = 20) String ad, String renk) {
}
