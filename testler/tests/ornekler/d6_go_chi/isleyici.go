package main

import (
	"net/http"
	"strconv"

	"example.com/blog/veri"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/render"
)

type Isleyici struct {
	depo *veri.Depo
}

type MakaleGirdisi struct {
	Baslik string `json:"baslik"`
	Govde  string `json:"govde"`
}

func (h *Isleyici) makaleListele(w http.ResponseWriter, r *http.Request) {
	etiket := r.URL.Query().Get("etiket")
	liste := h.depo.Listele(etiket)
	render.JSON(w, r, liste)
}

func (h *Isleyici) makaleEkle(w http.ResponseWriter, r *http.Request) {
	var g MakaleGirdisi
	if err := render.DecodeJSON(r.Body, &g); err != nil {
		render.Status(r, http.StatusBadRequest)
		return
	}
	h.depo.Ekle(g.Baslik, g.Govde)
}

func (h *Isleyici) makaleGetir(w http.ResponseWriter, r *http.Request) {
	id, _ := strconv.Atoi(chi.URLParam(r, "makaleID"))
	m, err := h.depo.Getir(id)
	if err != nil {
		http.Error(w, "yok", http.StatusNotFound)
		return
	}
	render.JSON(w, r, m)
}

func (h *Isleyici) makaleGuncelle(w http.ResponseWriter, r *http.Request) {}

func (h *Isleyici) akis(w http.ResponseWriter, r *http.Request) {}

func (h *Isleyici) pano(w http.ResponseWriter, r *http.Request) {}

func (h *Isleyici) makaleSil(w http.ResponseWriter, r *http.Request) {
	h.depo.Sil(chi.URLParam(r, "id"))
	w.WriteHeader(http.StatusNoContent)
}
