using System.ComponentModel.DataAnnotations.Schema;

namespace Notlar.Api.Modeller;

[Table("notlar")]
public class Not
{
    public int Id { get; set; }
    public bool Arsiv { get; set; }
}
