package main

import (
	"example.com/notlar/internal/handlers"
	"example.com/notlar/internal/service"
	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

func main() {
	var db *gorm.DB
	svc := service.NewItemService(db)
	h := handlers.NewItemHandler(svc)
	r := gin.Default()
	r.DELETE("/items/:id", h.Delete)
	r.Run(":8080")
}
