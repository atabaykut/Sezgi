package com.ornek.notlar.service;

import com.ornek.notlar.domain.Note;
import com.ornek.notlar.repo.NoteAuditMapper;
import com.ornek.notlar.repo.NoteRepository;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.access.AccessDeniedException;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.stereotype.Service;

@Service
public class NoteServiceImpl implements NoteService {
    @Autowired
    private NoteRepository noteRepository;

    @Autowired
    private NoteAuditMapper noteAuditMapper;

    @Override
    @PreAuthorize("hasPermission(#id, 'Note', 'delete')")
    public void delete(Long id) {
        Note note = noteRepository.findById(id).orElseThrow();
        if (!note.getOwnerId().equals(currentUserId())) {
            throw new AccessDeniedException("not owner");
        }
        noteRepository.delete(note);
        noteAuditMapper.record(id);
    }

    private Long currentUserId() {
        return 1L;
    }
}
