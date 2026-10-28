package com.ornek.magaza.exception;

import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.ResponseStatus;

@ResponseStatus(HttpStatus.NOT_FOUND)
public class KayitYokException extends RuntimeException {
    public KayitYokException(String m) {
        super(m);
    }
}
