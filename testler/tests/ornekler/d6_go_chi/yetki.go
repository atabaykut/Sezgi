package main

import (
	"net/http"

	"github.com/go-chi/jwtauth/v5"
)

// YoneticiDenetimi yalnızca yönetici rolündeki belirtece izin verir
func YoneticiDenetimi(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, claims, _ := jwtauth.FromContext(r.Context())
		if claims == nil {
			http.Error(w, http.StatusText(401), 401)
			return
		}
		rol, _ := claims["rol"].(string)
		if rol != "yonetici" {
			http.Error(w, http.StatusText(403), 403)
			return
		}
		next.ServeHTTP(w, r)
	})
}
