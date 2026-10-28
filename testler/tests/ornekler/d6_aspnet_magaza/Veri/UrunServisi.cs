using Magaza.Api.Models;
using Microsoft.EntityFrameworkCore;

namespace Magaza.Api.Veri;

public interface IUrunServisi
{
    Task<List<UrunDto>> ListeleAsync(string? ara, int sayfa);
    Task<UrunDto?> GetirAsync(int id);
    Task<UrunDto> OlusturAsync(UrunOlustur girdi);
    Task GuncelleAsync(int id, UrunGuncelle girdi);
    Task SilAsync(int id, string kullanici);
}

public class UrunServisi : IUrunServisi
{
    private readonly MagazaDb _db;

    public UrunServisi(MagazaDb db) => _db = db;

    public Task<List<UrunDto>> ListeleAsync(string? ara, int sayfa) =>
        _db.Urunler.Where(u => ara == null || u.Ad.Contains(ara)).Skip((sayfa - 1) * 20).Take(20)
            .Select(u => new UrunDto(u.Id, u.Ad, u.Fiyat, UrunDurumu.Yayinda)).ToListAsync();

    public async Task<UrunDto?> GetirAsync(int id)
    {
        var u = await _db.Urunler.FindAsync(id);
        return u == null ? null : new UrunDto(u.Id, u.Ad, u.Fiyat, UrunDurumu.Yayinda);
    }

    public async Task<UrunDto> OlusturAsync(UrunOlustur girdi)
    {
        var kategori = await _db.Kategoriler.FindAsync(girdi.KategoriId) ?? throw new KeyNotFoundException("kategori yok");
        var urun = new Urun { Ad = girdi.Ad, Fiyat = girdi.Fiyat, KategoriId = kategori.Id };
        _db.Add(urun);
        await _db.SaveChangesAsync();
        return new UrunDto(urun.Id, urun.Ad, urun.Fiyat, girdi.Durum);
    }

    public async Task GuncelleAsync(int id, UrunGuncelle girdi)
    {
        var urun = await _db.Urunler.FirstOrDefaultAsync(u => u.Id == id);
        if (urun is null)
        {
            throw new KeyNotFoundException();
        }
        urun.Ad = girdi.Ad;
        _db.Urunler.Update(urun);
        await _db.SaveChangesAsync();
    }

    public async Task SilAsync(int id, string kullanici)
    {
        var urun = await _db.Urunler.FindAsync(id);
        if (urun == null)
        {
            throw new KeyNotFoundException();
        }
        if (urun.SahipId != kullanici)
        {
            throw new UnauthorizedAccessException("yalnızca sahibi silebilir");
        }
        _db.Urunler.Remove(urun);
        await _db.SaveChangesAsync();
    }
}
