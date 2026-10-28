import uuid

from sqlmodel import Field, SQLModel


class User(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    email: str


class ApiKey(SQLModel, table=True):
    id: str = Field(primary_key=True)


class Account(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    name: str
    balance: float = 0


class Transaction(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    account_id: uuid.UUID = Field(foreign_key="account.id")
    amount: float


class Budget(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    month: str


class BudgetCategory(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    budget_id: uuid.UUID = Field(foreign_key="budget.id")
    name: str


class Chat(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    title: str


class Message(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    chat_id: uuid.UUID = Field(foreign_key="chat.id")
    content: str


class ChatCreate(SQLModel):
    title: str


class MessageCreate(SQLModel):
    content: str


class AccountCreate(SQLModel):
    name: str


class AccountUpdate(SQLModel):
    name: str | None = None


class TransactionCreate(SQLModel):
    account_id: uuid.UUID
    amount: float


class TransactionQuery(SQLModel):
    text: str


class BudgetCreate(SQLModel):
    month: str


class CategoryCreate(SQLModel):
    name: str
