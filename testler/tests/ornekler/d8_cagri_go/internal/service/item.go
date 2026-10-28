package service

import (
	"errors"

	"example.com/notlar/internal/models"
	"gorm.io/gorm"
)

type ItemService struct {
	db *gorm.DB
}

func NewItemService(db *gorm.DB) *ItemService {
	return &ItemService{db: db}
}

func (s *ItemService) Delete(id string, userID string) error {
	var item models.Item
	s.db.First(&item, id)
	if item.OwnerID != userID {
		return errors.New("forbidden")
	}
	return s.db.Delete(&item).Error
}
