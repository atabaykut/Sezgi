using MediatR;
using Notlar.Api.Veri;

namespace Notlar.Api.Ozellikler;

public class ArsivleIstegi
{
    public record Komut(int Id) : IRequest<bool>;

    public class Isleyici : IRequestHandler<Komut, bool>
    {
        private readonly NotDb _db;

        public Isleyici(NotDb db)
        {
            _db = db;
        }

        public async Task<bool> Handle(Komut k, CancellationToken ct)
        {
            var n = await _db.Notlar.FindAsync(k.Id);
            _db.Notlar.Update(n!);
            await _db.SaveChangesAsync(ct);
            return true;
        }
    }
}
