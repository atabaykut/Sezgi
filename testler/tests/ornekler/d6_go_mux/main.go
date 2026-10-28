package main

import (
	"net/http"
	"os"

	"github.com/gorilla/handlers"
	"github.com/gorilla/mux"
)

type rota struct {
	Yontem   string
	Yol      string
	Isleyici http.HandlerFunc
}

var ekRotalar = []rota{
	{"GET", "/surum", surum},
	{Yontem: "POST", Yol: "/geri-bildirim", Isleyici: geriBildirim},
}

func main() {
	r := mux.NewRouter()
	r.HandleFunc("/saglik", saglik).Methods("GET")
	api := r.PathPrefix("/api/v1").Subrouter()
	api.Use(kimlikDogrula)
	api.HandleFunc("/gorevler", gorevler).Methods(http.MethodGet, http.MethodPost)
	api.HandleFunc("/gorevler/{id:[0-9]+}", gorev).Methods("GET")
	api.Methods("DELETE").Path("/gorevler/{id}").HandlerFunc(gorevSil)
	for _, rt := range ekRotalar {
		r.HandleFunc(rt.Yol, rt.Isleyici).Methods(rt.Yontem)
	}
	r.HandleFunc(os.Getenv("EK_YOL"), saglik)
	srv := &http.Server{Addr: ":8000", Handler: handlers.LoggingHandler(os.Stdout, r)}
	srv.ListenAndServe()
}

func kimlikDogrula(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		token := r.Header.Get("Authorization")
		if token == "" {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func saglik(w http.ResponseWriter, r *http.Request)       {}
func surum(w http.ResponseWriter, r *http.Request)        {}
func geriBildirim(w http.ResponseWriter, r *http.Request) {}
func gorevler(w http.ResponseWriter, r *http.Request)     {}
func gorevSil(w http.ResponseWriter, r *http.Request)     {}

func gorev(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	_ = vars["id"]
}

// baglanmamis hiçbir yerden çağrılmaz
func baglanmamis(r *mux.Router) {
	r.HandleFunc("/yetim", saglik)
}
