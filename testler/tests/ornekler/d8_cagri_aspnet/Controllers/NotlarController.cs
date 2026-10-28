using MediatR;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Notlar.Api.Ozellikler;
using Notlar.Api.Veri;

namespace Notlar.Api.Controllers;

[ApiController]
[Route("api/[controller]")]
public class NotlarController : ControllerBase
{
    private readonly INotServisi _servis;
    private readonly IMediator _mediator;

    public NotlarController(INotServisi servis, IMediator mediator)
    {
        _servis = servis;
        _mediator = mediator;
    }

    [HttpDelete("{id}")]
    [Authorize(Policy = "NotSahibi")]
    public async Task<IActionResult> Sil(int id)
    {
        await _servis.SilAsync(id);
        return NoContent();
    }

    [HttpPost("{id}/arsivle")]
    public async Task<IActionResult> Arsivle(int id) => Ok(await _mediator.Send(new ArsivleIstegi.Komut(id)));
}
