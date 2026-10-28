using System.ComponentModel.DataAnnotations;
using System.Text.Json.Serialization;

namespace Magaza.Api.Models;

public record UrunDto(int Id, string Ad, decimal Fiyat, UrunDurumu Durum);

public record UrunOlustur(
    [Required, StringLength(100, MinimumLength = 2)] string Ad,
    [Range(0.01, 100000)] decimal Fiyat,
    [EmailAddress] string? IletisimEposta,
    int KategoriId,
    UrunDurumu Durum = UrunDurumu.Taslak);

public class UrunGuncelle
{
    [Required]
    [MaxLength(100)]
    public string Ad { get; set; } = "";

    [Range(0, 1000)]
    public int Stok { get; set; }

    [RegularExpression(@"^[A-Z]{3}-\d{4}$")]
    [JsonPropertyName("sku_kodu")]
    public string? Sku { get; set; }

    public string Aciklama { get; set; } = "";

    public List<string>? Etiketler { get; set; }
}

public enum UrunDurumu
{
    Taslak,
    Yayinda,
    Arsiv
}

public static class Roller
{
    public const string Yonetici = "Yonetici";
    public const string Musteri = "Musteri";
}

public static class Politikalar
{
    public const string Yonetici = "YoneticiPolitikasi";
}
