package handlers

import (
	"net/http"
	"strconv"

	"example.com/kitapci/internal/models"
	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

type KitapHandler struct {
	db *gorm.DB
}

func NewKitapHandler(db *gorm.DB) *KitapHandler { return &KitapHandler{db: db} }

// KitapGirdisi kitap oluşturma gövdesi
type KitapGirdisi struct {
	Baslik    string   `json:"baslik" binding:"required,min=2,max=200"`
	Sayfa     int      `json:"sayfa" binding:"required,gte=1,lte=5000"`
	Tur       string   `json:"tur" binding:"required,oneof=roman siir"`
	Eposta    string   `json:"eposta,omitempty" binding:"omitempty,email"`
	Etiketler []string `json:"etiketler"`
	YazarID   uint     `json:"yazar_id" binding:"required"`
	Gizli     string   `json:"-"`
	Yayinci   *Yayinci `json:"yayinci"`
}

type Yayinci struct {
	Ad   string `json:"ad" binding:"required"`
	Ulke string `json:"ulke"`
}

type KitapYaniti struct {
	ID     uint   `json:"id"`
	Baslik string `json:"baslik"`
	Ozet   string `json:"ozet,omitempty"`
}

// Listele godoc
// @Summary Kitapları listele
// @ID kitap-listele
func (h *KitapHandler) Listele(c *gin.Context) {
	sayfa, _ := strconv.Atoi(c.DefaultQuery("sayfa", "1"))
	q := c.Query("q")
	var kitaplar []models.Kitap
	h.db.Where("baslik LIKE ?", q).Offset(sayfa).Find(&kitaplar)
	c.JSON(http.StatusOK, kitaplar)
}

// Getir bir kitabı döndürür.
func (h *KitapHandler) Getir(c *gin.Context) {
	id, err := strconv.Atoi(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"hata": "geçersiz kimlik"})
		return
	}
	var k models.Kitap
	if err := h.db.First(&k, id).Error; err != nil {
		c.AbortWithStatusJSON(http.StatusNotFound, gin.H{"hata": "yok"})
		return
	}
	c.JSON(http.StatusOK, KitapYaniti{ID: k.ID, Baslik: k.Baslik})
}

func (h *KitapHandler) Olustur(c *gin.Context) {
	var girdi KitapGirdisi
	if err := c.ShouldBindJSON(&girdi); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"hata": err.Error()})
		return
	}
	k := models.Kitap{Baslik: girdi.Baslik}
	h.db.Create(&k)
	c.JSON(http.StatusCreated, KitapYaniti{ID: k.ID, Baslik: k.Baslik})
}

func (h *KitapHandler) Guncelle(c *gin.Context) {
	var girdi KitapGirdisi
	if err := c.ShouldBindJSON(&girdi); err != nil {
		c.AbortWithStatus(http.StatusBadRequest)
		return
	}
	userID := c.GetString("kullanici")
	var k models.Kitap
	h.db.First(&k, c.Param("id"))
	if k.OwnerID != userID {
		c.AbortWithStatus(http.StatusForbidden)
		return
	}
	h.db.Model(&models.Kitap{}).Where("id = ?", k.ID).Updates(map[string]any{"baslik": girdi.Baslik})
	c.Status(http.StatusNoContent)
}

// Sil kitabı siler.
// Deprecated: yerine arşivleme kullanılmalı
func (h *KitapHandler) Sil(c *gin.Context) {
	h.kitapSil(c.Param("id"))
	c.Status(http.StatusNoContent)
}

func (h *KitapHandler) kitapSil(id string) {
	h.db.Delete(&models.Kitap{}, id)
}
