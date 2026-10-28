package com.ornek.magaza.dto;

import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;

public record GirisIstegi(@NotBlank @Email String eposta, @NotBlank @Size(min = 8, max = 64) String sifre) {
}
