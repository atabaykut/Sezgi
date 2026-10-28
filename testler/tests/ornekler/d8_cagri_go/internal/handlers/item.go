package handlers

import (
	"example.com/notlar/internal/service"
	"github.com/gin-gonic/gin"
)

type ItemHandler struct {
	svc *service.ItemService
}

func NewItemHandler(svc *service.ItemService) *ItemHandler {
	return &ItemHandler{svc: svc}
}

func (h *ItemHandler) Delete(c *gin.Context) {
	if err := h.svc.Delete(c.Param("id"), c.GetString("userID")); err != nil {
		c.JSON(403, gin.H{"error": err.Error()})
		return
	}
	c.Status(204)
}
