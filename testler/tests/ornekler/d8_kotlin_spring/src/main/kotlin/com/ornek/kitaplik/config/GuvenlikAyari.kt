package com.ornek.kitaplik.config

import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.http.HttpMethod
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity
import org.springframework.security.config.annotation.web.builders.HttpSecurity
import org.springframework.security.config.annotation.web.invoke
import org.springframework.security.web.SecurityFilterChain

@Configuration
@EnableMethodSecurity
class GuvenlikAyari {
    @Bean
    fun filterChain(http: HttpSecurity): SecurityFilterChain {
        http {
            csrf { disable() }
            authorizeHttpRequests {
                authorize(HttpMethod.GET, "/api/kitaplar/**", permitAll)
                authorize("/api/auth/**", permitAll)
                authorize("/api/yonetim/**", hasRole("ADMIN"))
                authorize(anyRequest, authenticated)
            }
            httpBasic { }
        }
        return http.build()
    }
}
