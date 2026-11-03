async def koruma(request, call_next):
    if not request.cookies.get("oturum"):
        from fastapi.responses import PlainTextResponse
        return PlainTextResponse("giris gerekli", status_code=401)
    return await call_next(request)
