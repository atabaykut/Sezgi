using Magaza.Api.Models;
using Microsoft.EntityFrameworkCore;

namespace Magaza.Api.Veri;

public class MagazaDb : DbContext
{
    public MagazaDb(DbContextOptions<MagazaDb> options) : base(options) { }

    public DbSet<Kategori> Kategoriler => Set<Kategori>();
    public DbSet<Urun> Urunler { get; set; } = null!;
    public DbSet<Yorum> Yorumlar { get; set; } = null!;
    public DbSet<Not> Notlar { get; set; } = null!;
    public DbSet<Siparis> Siparisler { get; set; } = null!;

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<Urun>()
            .HasOne(u => u.Kategori)
            .WithMany(k => k.Urunler)
            .HasForeignKey(u => u.KategoriId)
            .OnDelete(DeleteBehavior.Cascade);

        modelBuilder.Entity<Yorum>(e =>
        {
            e.HasOne(y => y.Urun).WithMany(u => u.Yorumlar).OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<Siparis>().ToTable("siparisler")
            .HasOne(s => s.Urun).WithMany().OnDelete(DeleteBehavior.Restrict);
    }
}
