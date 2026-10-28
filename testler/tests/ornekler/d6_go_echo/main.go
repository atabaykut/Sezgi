package main

import (
	"net/http"

	"github.com/go-playground/validator/v10"
	echojwt "github.com/labstack/echo-jwt/v4"
	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

type Dogrulayici struct{ v *validator.Validate }

func (d *Dogrulayici) Validate(i interface{}) error { return d.v.Struct(i) }

type KayitGirdisi struct {
	Eposta string `json:"eposta" validate:"required,email"`
	Sifre  string `json:"sifre" validate:"required,min=8"`
	Rol    string `json:"rol" validate:"omitempty,oneof=uye yonetici"`
	Yas    int    `json:"yas" validate:"gt=17"`
}

type SiparisSorgusu struct {
	ID    int    `param:"id"`
	Durum string `query:"durum"`
	Not   string `json:"not"`
}

type Kullanici struct {
	ID     int    `json:"id"`
	Eposta string `json:"eposta"`
}

func main() {
	e := echo.New()
	e.Validator = &Dogrulayici{v: validator.New()}
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())
	e.POST("/kayit", kayit)
	api := e.Group("/api")
	api.Use(echojwt.JWT([]byte("gizli")))
	api.GET("/siparisler/:id", siparis)
	api.PUT("/siparisler/:id", siparis)
	yonetim := api.Group("/yonetim", yoneticiMi)
	yonetim.DELETE("/kullanicilar/:id", kullaniciSil)
	e.GET("/kamu/:dosya", dosya, ozelKontrol)
	e.Logger.Fatal(e.Start(":1323"))
}

func kayit(c echo.Context) error {
	var g KayitGirdisi
	if err := c.Bind(&g); err != nil {
		return echo.NewHTTPError(http.StatusBadRequest, err.Error())
	}
	if err := c.Validate(&g); err != nil {
		return err
	}
	return c.JSON(http.StatusCreated, Kullanici{Eposta: g.Eposta})
}

func siparis(c echo.Context) error {
	var s SiparisSorgusu
	if err := c.Bind(&s); err != nil {
		return echo.ErrBadRequest
	}
	return c.JSON(http.StatusOK, map[string]any{"id": s.ID, "durum": s.Durum})
}

func yoneticiMi(next echo.HandlerFunc) echo.HandlerFunc {
	return func(c echo.Context) error {
		rol, _ := c.Get("rol").(string)
		if rol != "yonetici" {
			return echo.ErrForbidden
		}
		return next(c)
	}
}

func kullaniciSil(c echo.Context) error {
	return c.NoContent(http.StatusNoContent)
}

func dosya(c echo.Context) error {
	return c.File(c.Param("dosya"))
}
