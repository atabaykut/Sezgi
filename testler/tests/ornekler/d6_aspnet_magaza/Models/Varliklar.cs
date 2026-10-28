using System.ComponentModel.DataAnnotations;
using System.ComponentModel.DataAnnotations.Schema;

namespace Magaza.Api.Models;

public class Kategori
{
    public int Id { get; set; }
    public string Ad { get; set; } = "";
    public List<Urun> Urunler { get; set; } = new();
}

[Table("urun_tablosu")]
public class Urun
{
    public int Id { get; set; }
    public string Ad { get; set; } = "";
    public decimal Fiyat { get; set; }
    public string SahipId { get; set; } = "";
    public bool IsDeleted { get; set; }
    public int KategoriId { get; set; }
    public Kategori Kategori { get; set; } = null!;
    public List<Yorum> Yorumlar { get; set; } = new();
}

public class Yorum
{
    public int Id { get; set; }
    public string Metin { get; set; } = "";
    public int UrunId { get; set; }
    public Urun Urun { get; set; } = null!;
}

public class Not
{
    public int Id { get; set; }
    public string Baslik { get; set; } = "";
    public string? Icerik { get; set; }
}

public class Siparis
{
    public Guid Id { get; set; }
    public int? UrunId { get; set; }
    public Urun? Urun { get; set; }
    public int Adet { get; set; }
}
