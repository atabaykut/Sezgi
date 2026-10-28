package main

import (
	"github.com/go-playground/validator/v10"
	"github.com/gofiber/fiber/v2"
)

var dogrula = validator.New()

type GirisGirdisi struct {
	Kullanici string `json:"kullanici" validate:"required"`
	Sifre     string `json:"sifre" validate:"required,min=6"`
}

type NotGirdisi struct {
	Baslik  string   `json:"baslik" validate:"required,max=120"`
	Icerik  string   `json:"icerik"`
	Etiketler []string `json:"etiketler" validate:"max=5"`
}

type Not struct {
	ID     int    `json:"id"`
	Baslik string `json:"baslik"`
}

func giris(c *fiber.Ctx) error {
	var g GirisGirdisi
	if err := c.BodyParser(&g); err != nil {
		return fiber.NewError(fiber.StatusBadRequest, "geçersiz")
	}
	if err := dogrula.Struct(g); err != nil {
		return c.Status(fiber.StatusUnprocessableEntity).JSON(fiber.Map{"hata": err.Error()})
	}
	return c.JSON(fiber.Map{"belirtec": "x"})
}

func profil(c *fiber.Ctx) error {
	return c.JSON(Not{})
}

func notGetir(c *fiber.Ctx) error {
	id, _ := c.ParamsInt("id")
	if id == 0 {
		return fiber.ErrNotFound
	}
	return c.JSON(Not{ID: id})
}

func notEkle(c *fiber.Ctx) error {
	var g NotGirdisi
	if err := c.BodyParser(&g); err != nil {
		return err
	}
	if err := dogrula.Struct(&g); err != nil {
		return fiber.NewError(fiber.StatusBadRequest, err.Error())
	}
	etiket := c.Query("etiket", "genel")
	_ = etiket
	return c.Status(fiber.StatusCreated).JSON(Not{Baslik: g.Baslik})
}

func notSil(c *fiber.Ctx) error {
	return c.SendStatus(fiber.StatusNoContent)
}
