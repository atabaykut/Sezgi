using Magaza.Api.Veri;
using Microsoft.AspNetCore.Mvc;

namespace Magaza.Api.Uclar;

public static class KampanyaUclari
{
    public static IEndpointRouteBuilder MapKampanyaUclari(this IEndpointRouteBuilder app)
    {
        var grup = app.MapGroup("/api/kampanyalar").AllowAnonymous();
        grup.MapGet("/{kod}", Getir);
        grup.MapPost("/{kod}/katil", [Authorize(Roles = "Musteri")] (string kod, [FromBody] KatilimGirdisi girdi) => Results.Accepted());
        return app;
    }

    static IResult Getir(string kod) => kod.Length > 0 ? Results.Ok(kod) : Results.NotFound();
}

public record KatilimGirdisi(
    [property: Required, EmailAddress] string Eposta,
    [property: Range(1, 10)] int Adet);
