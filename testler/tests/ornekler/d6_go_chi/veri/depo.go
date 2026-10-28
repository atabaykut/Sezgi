package veri

import "github.com/jmoiron/sqlx"

type Makale struct {
	ID     int    `db:"id" json:"id"`
	Baslik string `db:"baslik" json:"baslik"`
	Govde  string `db:"govde" json:"govde,omitempty"`
}

type Depo struct {
	db *sqlx.DB
}

func YeniDepo(db *sqlx.DB) *Depo { return &Depo{db: db} }

func (d *Depo) Listele(etiket string) []Makale {
	var l []Makale
	d.db.Select(&l, "SELECT * FROM makaleler WHERE etiket = $1", etiket)
	return l
}

func (d *Depo) Getir(id int) (*Makale, error) {
	var m Makale
	err := d.db.Get(&m, makaleSorgusu, id)
	return &m, err
}

const makaleSorgusu = `-- name: MakaleGetir :one
SELECT id, baslik, govde FROM makaleler WHERE id = $1`

func (d *Depo) Ekle(baslik, govde string) {
	d.db.MustExec("INSERT INTO makaleler (baslik, govde) VALUES ($1, $2)", baslik, govde)
}

func (d *Depo) Sil(id string) {
	d.db.MustExec("DELETE FROM makaleler WHERE id = $1", id)
	d.db.MustExec("DELETE FROM yorumlar WHERE makale_id = $1", id)
}
