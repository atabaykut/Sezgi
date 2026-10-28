using System.Security.Claims;
using Magaza.Api.Models;
using Magaza.Api.Ozellikler;
using MediatR;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

namespace Magaza.Api.Controllers;

/// <summary>Ürün kataloğu</summary>
[ApiController]
[Route("api/[controller]")]
public class UrunlerController : ControllerBase
{
    private readonly IUrunServisi _servis;
    private readonly IMediator _mediator;

    public UrunlerController(IUrunServisi servis, IMediator mediator)
    {
        _servis = servis;
        _mediator = mediator;
    }

    /// <summary>Ürünleri listeler</summary>
    [HttpGet]
    [AllowAnonymous]
    public async Task<ActionResult<List<UrunDto>>> Listele([FromQuery] string? ara, [FromQuery] int sayfa = 1)
    {
        return Ok(await _servis.ListeleAsync(ara, sayfa));
    }

    [HttpGet("{id:int}", Name = "UrunGetir")]
    [AllowAnonymous]
    public async Task<ActionResult<UrunDto>> GetirAsync(int id)
    {
        var urun = await _servis.GetirAsync(id);
        if (urun == null)
        {
            return NotFound();
        }
        return urun;
    }

    [HttpPost]
    [Authorize(Roles = "Yonetici,Editor")]
    [ProducesResponseType(typeof(UrunDto), StatusCodes.Status201Created)]
    public async Task<IActionResult> Olustur([FromBody] UrunOlustur girdi)
    {
        var dto = await _servis.OlusturAsync(girdi);
        await _mediator.Publish(new UrunOlusturuldu(dto.Id));
        return CreatedAtAction(nameof(GetirAsync), new { id = dto.Id }, dto);
    }

    [HttpPut("{id:int}")]
    public async Task<IActionResult> Guncelle(int id, UrunGuncelle girdi)
    {
        await _servis.GuncelleAsync(id, girdi);
        return NoContent();
    }

    [HttpDelete("{id:int}")]
    [Authorize(Policy = Politikalar.Yonetici)]
    public async Task<IActionResult> Sil(int id)
    {
        var kullanici = User.FindFirstValue(ClaimTypes.NameIdentifier);
        await _servis.SilAsync(id, kullanici!);
        return NoContent();
    }
}
