package com.ornek.magaza.web;

import com.ornek.magaza.exception.StokYokException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

@RestControllerAdvice
public class HataYakalayici {
    @ExceptionHandler(StokYokException.class)
    public ResponseEntity<String> stokYok(StokYokException e) {
        return ResponseEntity.status(HttpStatus.CONFLICT).body(e.getMessage());
    }
}
