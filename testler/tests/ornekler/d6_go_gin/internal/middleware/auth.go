package middleware

import (
	"net/http"

	"github.com/gin-gonic/gin"
)

// Kimlik Authorization başlığı olmayan isteği reddeder
func Kimlik() gin.HandlerFunc {
	return func(c *gin.Context) {
		belirtec := c.GetHeader("Authorization")
		if belirtec == "" {
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"hata": "giriş gerekli"})
			return
		}
		c.Set("kullanici", belirtec)
		c.Next()
	}
}

// RolIste kullanıcının rolü listede değilse 403 döner
func RolIste(roller ...string) gin.HandlerFunc {
	return func(c *gin.Context) {
		rol := c.GetString("rol")
		for _, r := range roller {
			if r == rol {
				c.Next()
				return
			}
		}
		c.AbortWithStatus(http.StatusForbidden)
	}
}
