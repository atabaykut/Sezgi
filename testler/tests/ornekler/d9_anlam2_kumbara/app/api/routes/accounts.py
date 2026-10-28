import uuid
from typing import Any

from fastapi import APIRouter

from app.api.deps import CurrentUser, SessionDep
from app.models import Account, AccountCreate, AccountUpdate

router = APIRouter(prefix="/accounts", tags=["accounts"])


@router.get("/")
def list_accounts(session: SessionDep, current_user: CurrentUser) -> Any:
    return session.query(Account).all()


@router.post("/")
def create_account(session: SessionDep, current_user: CurrentUser, account_in: AccountCreate) -> Any:
    account = Account(name=account_in.name)
    session.add(account)
    return account


@router.patch("/{id}")
def update_account(session: SessionDep, current_user: CurrentUser, id: uuid.UUID, account_in: AccountUpdate) -> Any:
    account = session.get(Account, id)
    session.add(account)
    return account


@router.delete("/{id}")
def delete_account(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    session.delete(session.get(Account, id))
    return {}
