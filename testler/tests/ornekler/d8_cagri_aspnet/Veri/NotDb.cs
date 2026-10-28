using Microsoft.EntityFrameworkCore;
using Notlar.Api.Modeller;

namespace Notlar.Api.Veri;

public class NotDb : DbContext
{
    public DbSet<Not> Notlar { get; set; } = null!;
}
