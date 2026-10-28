package main

import (
	"encoding/json"
	"net/http"
	"strconv"
)

type urunGirdisi struct {
	Ad    string  `json:"ad"`
	Fiyat float64 `json:"fiyat"`
	Stok  *int    `json:"stok,omitempty"`
}

type urun struct {
	ID    int     `json:"id"`
	Ad    string  `json:"ad"`
	Fiyat float64 `json:"fiyat"`
}

const urunSorgusu = `SELECT id, ad, fiyat FROM urunler WHERE id = $1`

func (s *sunucu) urunleriListele(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query()
	sayfa, _ := strconv.Atoi(q.Get("sayfa"))
	_ = sayfa
	rows, _ := s.db.Query("SELECT u.id, u.ad, k.ad FROM urunler u JOIN kategoriler k ON k.id = u.kategori_id")
	defer rows.Close()
	var liste []urun
	json.NewEncoder(w).Encode(liste)
}

func (s *sunucu) urunGetir(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.Atoi(r.PathValue("id"))
	if err != nil {
		http.Error(w, "geçersiz", http.StatusBadRequest)
		return
	}
	var u urun
	if err := s.db.QueryRow(urunSorgusu, id).Scan(&u.ID, &u.Ad, &u.Fiyat); err != nil {
		http.Error(w, "yok", http.StatusNotFound)
		return
	}
	json.NewEncoder(w).Encode(u)
}

func (s *sunucu) urunEkle(w http.ResponseWriter, r *http.Request) {
	var g urunGirdisi
	if err := json.NewDecoder(r.Body).Decode(&g); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	s.db.Exec("INSERT INTO urunler (ad, fiyat) VALUES ($1, $2)", g.Ad, g.Fiyat)
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(urun{Ad: g.Ad})
}

func (s *sunucu) urunSil(w http.ResponseWriter, r *http.Request) {
	s.db.Exec("DELETE FROM urunler WHERE id = $1", r.PathValue("id"))
	w.WriteHeader(http.StatusNoContent)
}

func (s *sunucu) eskiArama(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		r.URL.Query().Get("q")
	case http.MethodPost:
		r.FormValue("q")
	default:
		w.WriteHeader(http.StatusMethodNotAllowed)
	}
}

func (s *sunucu) dosya(w http.ResponseWriter, r *http.Request) {
	http.ServeFile(w, r, r.PathValue("yol"))
}

func (s *sunucu) anasayfa(w http.ResponseWriter, r *http.Request) {
	w.Write([]byte("merhaba"))
}

// rapor bir rapor oluşturur; dosya yüklemesi kabul eder
func (s *sunucu) rapor() http.HandlerFunc {
	tablo := "raporlar"
	return func(w http.ResponseWriter, r *http.Request) {
		f, _, err := r.FormFile("ek")
		if err != nil {
			http.Error(w, "dosya", http.StatusBadRequest)
			return
		}
		defer f.Close()
		s.db.Exec(dinamikSorgu(tablo))
		go raporUret(tablo)
	}
}

func dinamikSorgu(t string) string { return "INSERT INTO " + t }

func raporUret(t string) {}
