import uuid
from typing import Any

from fastapi import APIRouter

from app.api.deps import CurrentUser, SessionDep
from app.models import Budget, BudgetCategory, BudgetCreate, CategoryCreate

router = APIRouter(prefix="/budgets", tags=["budgets"])


@router.get("/")
def list_budgets(session: SessionDep, current_user: CurrentUser) -> Any:
    return session.query(Budget).all()


@router.post("/")
def create_budget(session: SessionDep, current_user: CurrentUser, budget_in: BudgetCreate) -> Any:
    budget = Budget(month=budget_in.month)
    session.add(budget)
    return budget


@router.post("/{id}/categories")
def add_budget_category(session: SessionDep, current_user: CurrentUser, id: uuid.UUID, category_in: CategoryCreate) -> Any:
    category = BudgetCategory(budget_id=id, name=category_in.name)
    session.add(category)
    return category
