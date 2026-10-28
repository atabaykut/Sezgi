using System.Text.Json.Serialization;
using Magaza.Api.Uclar;
using Magaza.Api.Veri;
using Microsoft.AspNetCore.Authorization;
using Microsoft.EntityFrameworkCore;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddControllers()
    .AddJsonOptions(o => o.JsonSerializerOptions.Converters.Add(new JsonStringEnumConverter()));
builder.Services.AddDbContext<MagazaDb>(o => o.UseSqlServer(builder.Configuration.GetConnectionString("Magaza")));
builder.Services.AddScoped<IUrunServisi, UrunServisi>();
builder.Services.AddMediatR(c => c.RegisterServicesFromAssemblyContaining<Program>());
builder.Services.AddHostedService<StokTemizleyici>();
builder.Services.AddAuthentication().AddJwtBearer();
builder.Services.AddAuthorization(o =>
{
    // Hiçbir yetki işareti olmayan uçlar da giriş ister
    o.FallbackPolicy = new AuthorizationPolicyBuilder().RequireAuthenticatedUser().Build();
    o.AddPolicy(Politikalar.Yonetici, p => p.RequireRole(Roller.Yonetici));
    o.AddPolicy("SiparisOkur", p => p.RequireClaim("izin", "siparis:oku"));
});

var app = builder.Build();

app.UseAuthentication();
app.UseAuthorization();

app.MapControllers();

app.MapGet("/saglik", () => Results.Ok(new { durum = "iyi" })).AllowAnonymous().WithName("Saglik");

var api = app.MapGroup("/api/v1");
var notlar = api.MapGroup("/notlar").RequireAuthorization().WithTags("Notlar");

notlar.MapGet("/", async (MagazaDb db) => TypedResults.Ok<List<Not>>(await db.Notlar.ToListAsync()));
notlar.MapGet("/{id:int}", async Task<Results<Ok<NotDto>, NotFound>> (int id, MagazaDb db) =>
    await db.Notlar.FindAsync(id) is Not n ? TypedResults.Ok(new NotDto(n.Id, n.Baslik)) : TypedResults.NotFound());
notlar.MapPost("/", async (NotGirdisi girdi, MagazaDb db) =>
{
    var not = new Not { Baslik = girdi.Baslik, Icerik = girdi.Icerik };
    db.Notlar.Add(not);
    await db.SaveChangesAsync();
    return Results.Created($"/api/v1/notlar/{not.Id}", not);
}).WithName("NotOlustur");
notlar.MapDelete("/{id:int}", NotUclari.Sil).RequireAuthorization(Politikalar.Yonetici);
notlar.MapGet("/ara", ([AsParameters] NotArama arama, MagazaDb db) => db.Notlar.Where(n => n.Baslik.Contains(arama.Metin)).ToList());

app.MapKampanyaUclari();

if (app.Environment.IsDevelopment())
{
    app.MapGet("/gelistirme/ayarlar", () => builder.Configuration.AsEnumerable());
}

app.Run();

public partial class Program { }
