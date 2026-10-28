import uuid
from typing import Any

from fastapi import APIRouter

from app.api.deps import CurrentUser, SessionDep
from app.models import Chat, ChatCreate, Message, MessageCreate

router = APIRouter(prefix="/chats", tags=["chats"])


@router.get("/")
def list_chats(session: SessionDep, current_user: CurrentUser) -> Any:
    return session.query(Chat).all()


@router.post("/")
def create_chat(session: SessionDep, current_user: CurrentUser, chat_in: ChatCreate) -> Any:
    chat = Chat(title=chat_in.title)
    session.add(chat)
    return chat


@router.get("/{id}")
def read_chat(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    return session.get(Chat, id)


@router.post("/{id}/messages")
def create_message(session: SessionDep, current_user: CurrentUser, id: uuid.UUID, message_in: MessageCreate) -> Any:
    message = Message(chat_id=id, content=message_in.content)
    session.add(message)
    return message


@router.delete("/{id}")
def delete_chat(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    session.delete(session.get(Chat, id))
    return {}
