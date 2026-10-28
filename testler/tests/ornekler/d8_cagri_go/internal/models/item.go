package models

type Item struct {
	ID      uint
	OwnerID string
}

func (Item) TableName() string {
	return "esyalar"
}
