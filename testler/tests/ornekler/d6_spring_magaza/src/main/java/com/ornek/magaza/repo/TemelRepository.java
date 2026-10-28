package com.ornek.magaza.repo;

import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.repository.NoRepositoryBean;

@NoRepositoryBean
public interface TemelRepository<T> extends JpaRepository<T, Long> {
}
