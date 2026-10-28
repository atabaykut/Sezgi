from fastapi import APIRouter

router = APIRouter(prefix="/deneme")


@router.post("/sifirla")
def sifirla():
    return {"tamam": True}
