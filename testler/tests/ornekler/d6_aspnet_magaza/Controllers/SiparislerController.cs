using Magaza.Api.Models;
using Magaza.Api.Ozellikler;
using MediatR;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

namespace Magaza.Api.Controllers;

[ApiController]
[ApiVersion("2.0")]
[Route("api/v{version:apiVersion}/[controller]")]
[Authorize(Policy = "SiparisOkur")]
public class SiparislerController(IMediator mediator) : ControllerBase
{
    [HttpGet("{id:guid}")]
    public async Task<ActionResult<SiparisDto>> Getir(Guid id) => await mediator.Send(new SiparisGetir(id));

    [HttpPost]
    [Authorize(Roles = Roller.Musteri)]
    public async Task<ActionResult<SiparisDto>> Ver(SiparisVer komut) => await mediator.Send(komut);

    [HttpPost("{id:guid}/iptal")]
    [Obsolete("Yerine DELETE kullanın")]
    public async Task<IActionResult> Iptal(Guid id, [FromHeader(Name = "X-Neden")] string? neden)
    {
        await mediator.Send(new SiparisIptal(id));
        return Ok();
    }
}
