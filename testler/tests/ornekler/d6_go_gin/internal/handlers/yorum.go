package handlers

import (
	"net/http"

	"example.com/kitapci/internal/middleware"
	"example.com/kitapci/internal/models"
	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

// YorumRotalari yorum uçlarını verilen gruba bağlar
func YorumRotalari(g *gin.RouterGroup, db *gorm.DB) {
	g.GET("", yorumlariListele(db))
	g.POST("/:kitap_id", middleware.Kimlik(), yorumEkle(db))
}

type YorumGirdisi struct {
	Metin string `json:"metin" binding:"required,max=500"`
	Puan  int    `json:"puan" binding:"min=1,max=5"`
}

func yorumlariListele(db *gorm.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var yorumlar []models.Yorum
		db.Find(&yorumlar)
		c.JSON(http.StatusOK, yorumlar)
	}
}

func yorumEkle(db *gorm.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var g YorumGirdisi
		if err := c.ShouldBindJSON(&g); err != nil {
			c.JSON(400, gin.H{"hata": "geçersiz"})
			return
		}
		db.Exec("INSERT INTO yorumlar (metin, kitap_id) VALUES (?, ?)", g.Metin, c.Param("kitap_id"))
		c.JSON(201, gin.H{"tamam": true})
	}
}

// KullanilmayanRotalar hiçbir yerden çağrılmaz: önekleri bilinmez
func KullanilmayanRotalar(r *gin.Engine) {
	r.GET("/sahipsiz", func(c *gin.Context) { c.String(200, "x") })
}
