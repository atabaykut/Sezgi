package main

import "net/http"

func (s *sunucu) kimlikIste(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") == "" {
			http.Error(w, "giriş gerekli", http.StatusUnauthorized)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (s *sunucu) yoneticiIste(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		rol := r.Header.Get("X-Rol")
		if rol != "yonetici" {
			http.Error(w, "yasak", http.StatusForbidden)
			return
		}
		next(w, r)
	}
}
