import uuid
from typing import Any

from fastapi import APIRouter

from app.api.deps import CurrentUser, SessionDep
from app.models import Album, AlbumCreate, AlbumUpdate, AlbumUsers

router = APIRouter(prefix="/albums", tags=["albums"])


@router.get("/")
def list_albums(session: SessionDep, current_user: CurrentUser) -> Any:
    """
    List albums.
    """
    return session.query(Album).all()


@router.post("/")
def create_album(session: SessionDep, current_user: CurrentUser, album_in: AlbumCreate) -> Any:
    """
    Create an album.
    """
    album = Album(title=album_in.title, owner_id=current_user.id)
    session.add(album)
    return album


@router.patch("/{id}")
def update_album(session: SessionDep, current_user: CurrentUser, id: uuid.UUID, album_in: AlbumUpdate) -> Any:
    """
    Update an album.
    """
    album = session.get(Album, id)
    session.add(album)
    return album


@router.put("/{id}/users")
def add_users_to_album(session: SessionDep, current_user: CurrentUser, id: uuid.UUID, body: AlbumUsers) -> Any:
    """
    Share an album with users.
    """
    album = session.get(Album, id)
    return album


@router.delete("/{id}")
def delete_album(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    """
    Delete an album.
    """
    album = session.get(Album, id)
    session.delete(album)
    return {"ok": True}
