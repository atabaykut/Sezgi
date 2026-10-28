from fastapi import APIRouter

router = APIRouter(prefix="/dinamik")
ADLAR = ["a", "b"]


def yap(ad: str):
    def uc():
        return {"ad": ad}

    return uc


for ad in ADLAR:
    router.add_api_route("/" + ad.upper(), yap(ad))
