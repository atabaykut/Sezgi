package com.ornek.magaza;

import org.springframework.web.bind.annotation.*;

@RestController
public class TestController {
    @GetMapping("/test-icin")
    public String t() {
        return "";
    }
}
