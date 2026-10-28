package models

import "gorm.io/gorm"

type Yazar struct {
	gorm.Model
	Ad       string
	Kitaplar []Kitap `gorm:"constraint:OnDelete:CASCADE"`
}

type Kitap struct {
	gorm.Model
	Baslik   string `gorm:"column:baslik;size:200"`
	OwnerID  string
	YazarID  uint
	Yazar    Yazar
	Yorumlar []Yorum `gorm:"constraint:OnDelete:CASCADE"`
}

type Yorum struct {
	ID      uint
	KitapID uint
	Metin   string
}

func (Yorum) TableName() string { return "yorumlar" }
