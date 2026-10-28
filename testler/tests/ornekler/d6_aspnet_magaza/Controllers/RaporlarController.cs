using Microsoft.AspNetCore.Mvc;

namespace Magaza.Api.Controllers;

[Route("api/[controller]/[action]")]
public class RaporlarController : ControllerBase
{
    [HttpGet]
    public IActionResult Gunluk([FromQuery] DateTime? tarih) => Ok();

    [HttpGet("{yil:int:min(2000)}")]
    [ActionName("Yillik")]
    public IActionResult YilSonu(int yil) => Ok();

    [HttpPost("~/api/disa-aktar")]
    public IActionResult DisaAktar(IFormFile dosya) => Accepted();
}
