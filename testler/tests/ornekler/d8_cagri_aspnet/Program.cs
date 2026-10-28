using Notlar.Api.Veri;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddControllers();
builder.Services.AddDbContext<NotDb>();
builder.Services.AddScoped<INotServisi, NotServisi>();
var app = builder.Build();
app.MapControllers();
app.Run();
