from fastapi import APIRouter, Depends

from app.api.deps import get_current_user

router = APIRouter(prefix="/yonetim", dependencies=[Depends(get_current_user)])


@router.get("/rapor")
def rapor():
    return {}


def temizle():
    return {}


router.add_api_route("/temizle", temizle, methods=["POST"])
