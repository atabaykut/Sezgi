from typing import Annotated, Any

from fastapi import APIRouter, Depends

from app.api.deps import SessionDep, require_api_key
from app.models import ApiKey, Transaction

router = APIRouter(prefix="/imports", tags=["imports"])


@router.post("/")
def create_import(session: SessionDep, key: Annotated[ApiKey, Depends(require_api_key)]) -> Any:
    session.add(Transaction(account_id=None, amount=0))
    return {}
