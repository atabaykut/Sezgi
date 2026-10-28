public class UclarTesti
{
    public void Deneme(WebApplication app) => app.MapGet("/test-ucu", () => "x");
}
