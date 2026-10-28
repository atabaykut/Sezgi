import uuid
from typing import Any

from fastapi import APIRouter

from app.api.deps import CurrentUser, SessionDep
from app.models import Transaction, TransactionCreate, TransactionQuery

router = APIRouter(prefix="/transactions", tags=["transactions"])


@router.get("/")
def list_transactions(session: SessionDep, current_user: CurrentUser) -> Any:
    return session.query(Transaction).all()


@router.post("/")
def create_transaction(session: SessionDep, current_user: CurrentUser, tx_in: TransactionCreate) -> Any:
    tx = Transaction(account_id=tx_in.account_id, amount=tx_in.amount)
    session.add(tx)
    return tx


@router.post("/search")
def search_transactions(session: SessionDep, current_user: CurrentUser, q: TransactionQuery) -> Any:
    return session.query(Transaction).all()


@router.delete("/{id}")
def delete_transaction(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    session.delete(session.get(Transaction, id))
    return {}
