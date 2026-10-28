package com.ornek.kitaplik

import org.springframework.web.bind.annotation.*

@RestController
class TestController {
    @GetMapping("/yalnizca-test")
    fun t() = "t"
}
