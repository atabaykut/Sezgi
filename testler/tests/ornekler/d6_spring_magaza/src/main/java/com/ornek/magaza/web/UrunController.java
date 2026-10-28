package com.ornek.magaza.web;

import com.ornek.magaza.config.ApiYollari;
import com.ornek.magaza.dto.UrunIstegi;
import com.ornek.magaza.dto.UrunYaniti;
import com.ornek.magaza.exception.KayitYokException;
import com.ornek.magaza.repo.UrunRepository;
import com.ornek.magaza.service.UrunService;
import jakarta.validation.Valid;
import java.util.List;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping(ApiYollari.URUNLER)
public class UrunController {

    private final UrunService urunService;
    private final UrunRepository urunRepository;

    public UrunController(UrunService urunService, UrunRepository urunRepository) {
        this.urunService = urunService;
        this.urunRepository = urunRepository;
    }

    /** Ürünleri arar. Sayfalıdır. */
    @GetMapping
    public List<UrunYaniti> listele(@RequestParam(required = false) String q, @RequestParam(defaultValue = "0") int sayfa, @RequestParam("adet") Integer adet) {
        return urunService.ara(q, sayfa);
    }

    @GetMapping("/{id}")
    public ResponseEntity<UrunYaniti> getir(@PathVariable Long id) {
        var urun = urunRepository.findById(id).orElseThrow(() -> new KayitYokException("ürün yok"));
        return ResponseEntity.ok(UrunYaniti.from(urun));
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    public UrunYaniti olustur(@Valid @RequestBody UrunIstegi istek) {
        return urunService.olustur(istek);
    }

    @PutMapping("/{id}")
    public UrunYaniti guncelle(@PathVariable("id") Long urunId, @RequestBody UrunIstegi istek) {
        return urunService.guncelle(urunId, istek);
    }

    @DeleteMapping("/{id:\d+}")
    @PreAuthorize("hasRole('ADMIN')")
    public void sil(@PathVariable Long id) {
        urunService.sil(id);
    }
}
