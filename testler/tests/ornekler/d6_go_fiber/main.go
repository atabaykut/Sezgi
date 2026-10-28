package main

import (
	jwtware "github.com/gofiber/contrib/jwt"
	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/logger"
)

func main() {
	app := fiber.New()
	app.Use(logger.New())
	app.Get("/durum", func(c *fiber.Ctx) error {
		return c.SendString("ok")
	})
	api := app.Group("/api/v2")
	api.Post("/giris", giris)
	api.Use(jwtware.New(jwtware.Config{SigningKey: jwtware.SigningKey{Key: []byte("gizli")}}))
	api.Get("/profil", profil)
	notlar := api.Group("/notlar")
	notlar.Get("/:id<int>", notGetir)
	notlar.Post("/", notEkle)
	notlar.Delete("/:id", notSil)
	app.Listen(":3000")
}
