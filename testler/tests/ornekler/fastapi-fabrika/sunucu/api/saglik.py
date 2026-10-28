from fastapi import APIRouter

router = APIRouter(prefix="/saglik")


@router.get("/")
def kontrol():
    return {"ok": True}
