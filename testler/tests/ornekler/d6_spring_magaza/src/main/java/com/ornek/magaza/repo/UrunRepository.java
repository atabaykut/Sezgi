package com.ornek.magaza.repo;

import com.ornek.magaza.domain.Urun;
import java.util.List;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;

public interface UrunRepository extends TemelRepository<Urun> {
    List<Urun> findByAdContaining(String q);

    @Modifying
    @Query("delete from Urun u where u.stok = 0")
    int stoksuzlariSil();
}
