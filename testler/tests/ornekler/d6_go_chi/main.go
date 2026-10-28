package main

import (
	"net/http"

	"example.com/blog/veri"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-chi/jwtauth/v5"
)

var tokenAuth = jwtauth.New("HS256", []byte("gizli"), nil)

func main() {
	depo := veri.YeniDepo(nil)
	h := &Isleyici{depo: depo}
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Get("/", anasayfa)
	r.Route("/makaleler", func(r chi.Router) {
		r.Get("/", h.makaleListele)
		r.Post("/", h.makaleEkle)
		r.Route("/{makaleID:[0-9]+}", func(r chi.Router) {
			r.Get("/", h.makaleGetir)
			r.With(jwtauth.Verifier(tokenAuth), jwtauth.Authenticator(tokenAuth)).Put("/", h.makaleGuncelle)
		})
	})
	r.Group(func(r chi.Router) {
		r.Use(jwtauth.Verifier(tokenAuth))
		r.Get("/akis", h.akis)
	})
	r.Mount("/yonetim", yonetimRouter(h))
	http.ListenAndServe(":3333", r)
}

func yonetimRouter(h *Isleyici) http.Handler {
	r := chi.NewRouter()
	r.Use(YoneticiDenetimi)
	r.Get("/", h.pano)
	r.Delete("/makaleler/{id}", h.makaleSil)
	return r
}

func anasayfa(w http.ResponseWriter, r *http.Request) {
	w.Write([]byte("blog"))
}
