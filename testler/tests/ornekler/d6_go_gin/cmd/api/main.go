package main

import (
	"example.com/kitapci/internal/handlers"
	"example.com/kitapci/internal/middleware"
	"example.com/kitapci/internal/models"
	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

const apiOnek = "/api"

func main() {
	var db *gorm.DB
	db.AutoMigrate(&models.Yazar{}, &models.Kitap{}, &models.Yorum{})
	r := gin.Default()
	r.Use(gin.Logger())
	r.GET("/saglik", func(c *gin.Context) {
		c.JSON(200, gin.H{"durum": "ok", "surum": "1"})
	})
	h := handlers.NewKitapHandler(db)
	v1 := r.Group(apiOnek + "/v1")
	{
		v1.GET("/kitaplar", h.Listele)
		v1.GET("/kitaplar/:id", h.Getir)
		yetkili := v1.Group("", middleware.Kimlik())
		yetkili.POST("/kitaplar", h.Olustur)
		yetkili.PUT("/kitaplar/:id", h.Guncelle)
		yonetim := yetkili.Group("/yonetim", middleware.RolIste("admin", "editor"))
		yonetim.DELETE("/kitaplar/:id", h.Sil)
	}
	handlers.YorumRotalari(v1.Group("/yorumlar"), db)
	if gin.Mode() == gin.DebugMode {
		r.GET("/hata-ayikla", h.Listele)
	}
	r.Run(":8080")
}
