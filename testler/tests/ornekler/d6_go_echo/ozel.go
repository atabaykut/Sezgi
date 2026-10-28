package main

import "github.com/labstack/echo/v4"

func ozelKontrol(next echo.HandlerFunc) echo.HandlerFunc {
	return func(c echo.Context) error {
		c.Response().Header().Set("X-Ornek", "1")
		return next(c)
	}
}
