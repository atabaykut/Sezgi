package com.ornek.notlar.repo;

import org.apache.ibatis.annotations.Mapper;

@Mapper
public interface NoteAuditMapper {
    void record(Long noteId);
}
