using Magaza.Api.Models;
using Magaza.Api.Veri;
using MediatR;
using Microsoft.EntityFrameworkCore;

namespace Magaza.Api.Ozellikler;

public record SiparisDto(Guid Id, int Adet);

public record SiparisGetir(Guid Id) : IRequest<SiparisDto>;

public record SiparisVer([Required] int UrunId, [Range(1, 99)] int Adet) : IRequest<SiparisDto>;

public record SiparisIptal(Guid Id) : IRequest;

public record UrunOlusturuldu(int UrunId) : INotification;

public class SiparisGetirIsleyici(MagazaDb db) : IRequestHandler<SiparisGetir, SiparisDto>
{
    public async Task<SiparisDto> Handle(SiparisGetir istek, CancellationToken ct)
    {
        var s = await db.Siparisler.FirstOrDefaultAsync(x => x.Id == istek.Id, ct);
        if (s == null)
        {
            throw new KeyNotFoundException();
        }
        return new SiparisDto(s.Id, s.Adet);
    }
}

public class SiparisVerIsleyici(MagazaDb db, IPublisher yayinci) : IRequestHandler<SiparisVer, SiparisDto>
{
    public async Task<SiparisDto> Handle(SiparisVer istek, CancellationToken ct)
    {
        var s = new Siparis { Id = Guid.NewGuid(), UrunId = istek.UrunId, Adet = istek.Adet };
        db.Siparisler.Add(s);
        await db.SaveChangesAsync(ct);
        await yayinci.Publish(new SiparisVerildi(s.Id), ct);
        return new SiparisDto(s.Id, s.Adet);
    }
}

public class SiparisIptalIsleyici(MagazaDb db) : IRequestHandler<SiparisIptal>
{
    public async Task Handle(SiparisIptal istek, CancellationToken ct)
    {
        await db.Siparisler.Where(x => x.Id == istek.Id).ExecuteDeleteAsync(ct);
    }
}

public record SiparisVerildi(Guid SiparisId) : INotification;

public class StokDusur(MagazaDb db) : INotificationHandler<SiparisVerildi>
{
    public async Task Handle(SiparisVerildi olay, CancellationToken ct)
    {
        var s = await db.Siparisler.FindAsync(olay.SiparisId);
        db.Urunler.Remove(new Urun { Id = s!.UrunId ?? 0 });
        await db.SaveChangesAsync(ct);
    }
}

public class UrunBildirimi : INotificationHandler<UrunOlusturuldu>
{
    public Task Handle(UrunOlusturuldu olay, CancellationToken ct) => Task.CompletedTask;
}

public class StokTemizleyici(IServiceScopeFactory kapsam) : BackgroundService
{
    protected override async Task ExecuteAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            using var k = kapsam.CreateScope();
            var db = k.ServiceProvider.GetRequiredService<MagazaDb>();
            await db.Yorumlar.Where(y => y.Metin == "").ExecuteDeleteAsync(ct);
            await Task.Delay(TimeSpan.FromHours(1), ct);
        }
    }
}
