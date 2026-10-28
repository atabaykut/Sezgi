namespace Notlar.Api.Veri;

public interface INotServisi
{
    Task SilAsync(int id);
}

public class NotServisi : INotServisi
{
    private readonly NotDb _db;

    public NotServisi(NotDb db)
    {
        _db = db;
    }

    public async Task SilAsync(int id)
    {
        var not = await _db.Notlar.FindAsync(id);
        _db.Notlar.Remove(not!);
        await _db.SaveChangesAsync();
    }
}
