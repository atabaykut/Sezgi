package com.ornek.magaza.repo;

import com.ornek.magaza.domain.Siparis;
import org.springframework.data.repository.CrudRepository;

public interface SiparisRepository extends CrudRepository<Siparis, Long> {
    Siparis findByNo(String no);
}
