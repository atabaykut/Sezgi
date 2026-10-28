package com.ornek.magaza.config;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.HttpMethod;
import org.springframework.security.config.Customizer;
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.web.SecurityFilterChain;

@Configuration
@EnableMethodSecurity
public class GuvenlikAyari {

    private static final String[] ACIK_YOLLAR = {"/api/auth/**", ApiYollari.SAGLIK};

    @Bean
    public SecurityFilterChain filterChain(HttpSecurity http) throws Exception {
        http.csrf(c -> c.disable())
            // Ürün kataloğu herkese açık; yönetim yalnızca ADMIN
            .authorizeHttpRequests(auth -> auth
                .requestMatchers(HttpMethod.GET, "/api/urunler/**").permitAll()
                .requestMatchers(ACIK_YOLLAR).permitAll()
                .requestMatchers("/api/yonetim/**").hasRole("ADMIN")
                .requestMatchers("/api/rapor/**").access(new OzelYetki())
                .anyRequest().authenticated())
            .httpBasic(Customizer.withDefaults());
        return http.build();
    }
}
