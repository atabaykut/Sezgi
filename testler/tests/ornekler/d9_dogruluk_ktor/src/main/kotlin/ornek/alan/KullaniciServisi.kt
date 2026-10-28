package ornek.alan

class KullaniciServisi(private val kullaniciDeposu: KullaniciDeposu) {
    fun profil(eposta: String, ad: String): Boolean {
        val k = kullaniciDeposu.adaGore(ad) ?: throw IllegalStateException("yok")
        return kullaniciDeposu.takipEdiyorMu(eposta, k)
    }

    fun takipEt(eposta: String, ad: String): Long = kullaniciDeposu.takipEt(eposta, ad)

    fun birak(eposta: String, ad: String): Long = kullaniciDeposu.birak(eposta, ad)
}
