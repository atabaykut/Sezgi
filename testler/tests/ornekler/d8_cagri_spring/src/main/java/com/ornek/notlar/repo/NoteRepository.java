package com.ornek.notlar.repo;

import com.ornek.notlar.domain.Note;
import org.springframework.data.jpa.repository.JpaRepository;

public interface NoteRepository extends JpaRepository<Note, Long> {
}
