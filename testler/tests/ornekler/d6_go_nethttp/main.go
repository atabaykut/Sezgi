package main

import (
	"database/sql"
	"log"
	"net/http"
)

type sunucu struct {
	db  *sql.DB
	mux *http.ServeMux
}

func yeniSunucu(db *sql.DB) *sunucu {
	s := &sunucu{db: db, mux: http.NewServeMux()}
	s.rotalar()
	return s
}

func (s *sunucu) rotalar() {
	s.mux.HandleFunc("GET /urunler", s.urunleriListele)
	s.mux.HandleFunc("GET /urunler/{id}", s.urunGetir)
	s.mux.Handle("POST /urunler", s.kimlikIste(http.HandlerFunc(s.urunEkle)))
	s.mux.HandleFunc("DELETE /urunler/{id}", s.yoneticiIste(s.urunSil))
	s.mux.HandleFunc("/eski/arama", s.eskiArama)
	s.mux.HandleFunc("GET /dosyalar/{yol...}", s.dosya)
	s.mux.HandleFunc("GET /{$}", s.anasayfa)
	s.mux.HandleFunc("POST /raporlar", s.rapor())
}

func main() {
	var db *sql.DB
	s := yeniSunucu(db)
	kok := http.NewServeMux()
	kok.Handle("/api/", http.StripPrefix("/api", s.mux))
	kok.HandleFunc("GET /saglik", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})
	http.HandleFunc("/varsayilan", s.anasayfa)
	log.Fatal(http.ListenAndServe(":8080", gunluk(kok)))
}

func gunluk(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		log.Println(r.URL.Path)
		next.ServeHTTP(w, r)
	})
}
