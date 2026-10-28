package main

import (
	"encoding/json"
	"net/http"

	"github.com/acme/kalkan"
	"github.com/gin-gonic/gin"
)

type CozucuGirdisi struct {
	Ad string `json:"ad" validate:"required"`
}

type KosulluGirdi struct {
	Tur    string `json:"tur" binding:"required"`
	Vergi  string `json:"vergi" binding:"required_if=Tur kurumsal"`
}

type KayitGirdisi struct {
	Eposta string `json:"eposta" binding:"required,email"`
	Yas    int    `json:"yas" binding:"gte=18"`
}

// Kimlik: zorunlu=false verilince hiçbir isteği reddetmez
func Kimlik(zorunlu bool) gin.HandlerFunc {
	return func(c *gin.Context) {
		if c.GetHeader("Authorization") == "" {
			if zorunlu {
				c.AbortWithStatus(http.StatusUnauthorized)
			}
			return
		}
		c.Set("kullanici", c.GetHeader("Authorization"))
	}
}

func main() {
	r := gin.New()
	// Tanımı görünmeyen dış ara katman: adı yetki söylemiyor ama kimlik isteyebilir
	r.GET("/a/kalkanli", kalkan.Koru(), bos)
	r.GET("/a/temel", gin.BasicAuth(gin.Accounts{"a": "b"}), bos)
	r.GET("/a/bayrakli", Kimlik(false), bos)
	r.POST("/a/cozucu", func(c *gin.Context) {
		var g CozucuGirdisi
		json.NewDecoder(c.Request.Body).Decode(&g)
	})
	r.POST("/a/kosullu", func(c *gin.Context) {
		var g KosulluGirdi
		c.ShouldBindJSON(&g)
	})
	r.POST("/a/kayit", func(c *gin.Context) {
		var g KayitGirdisi
		if err := c.ShouldBindJSON(&g); err != nil {
			c.AbortWithStatus(http.StatusBadRequest)
		}
	})
	r.Run()
}

func bos(c *gin.Context) {}
