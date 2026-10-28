using Magaza.Api.Veri;

namespace Magaza.Api.Uclar;

public static class NotUclari
{
    public static async Task<IResult> Sil(int id, MagazaDb db)
    {
        var not = await db.Notlar.FindAsync(id);
        if (not is null)
        {
            return Results.NotFound();
        }
        db.Notlar.Remove(not);
        await db.SaveChangesAsync();
        return Results.NoContent();
    }
}

public record NotDto(int Id, string Baslik);

public class NotGirdisi
{
    public required string Baslik { get; init; }
    public string? Icerik { get; init; }
}

public class NotArama
{
    [FromQuery(Name = "q")]
    public string Metin { get; set; } = "";
    public int Sayfa { get; set; } = 1;
}
