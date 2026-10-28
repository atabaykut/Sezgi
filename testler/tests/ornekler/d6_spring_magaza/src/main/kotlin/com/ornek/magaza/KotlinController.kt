package com.ornek.magaza

@RestController
class KotlinController {
    @GetMapping("/kotlin")
    fun k() = "k"
}
