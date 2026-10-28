package com.ornek.magaza.dto;

import com.fasterxml.jackson.annotation.JsonIgnore;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.ornek.magaza.domain.Kategori;
import jakarta.validation.Valid;
import jakarta.validation.constraints.*;
import java.math.BigDecimal;
import java.util.List;

public class UrunIstegi extends TemelIstek {
    private static final long serialVersionUID = 1L;

    @NotBlank
    @Size(min = 2, max = 100)
    private String ad;

    @NotNull
    @Positive
    private BigDecimal fiyat;

    @Min(0)
    @Max(1000)
    private Integer stok;

    private int oncelik;

    private Kategori kategori;

    @Valid
    @NotEmpty
    private List<EtiketIstegi> etiketler;

    @JsonProperty("sku_kodu")
    @Pattern(regexp = "[A-Z]{3}-\\d{4}")
    private String skuKodu;

    @Email
    private String iletisim;

    @Future
    private java.time.LocalDate yayinTarihi;

    @JsonIgnore
    private String ic;
}
