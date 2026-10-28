import uuid
from typing import Any

from fastapi import APIRouter, File, UploadFile

from app.api.deps import CurrentUser, SessionDep
from app.models import Photo

router = APIRouter(prefix="/photos", tags=["photos"])


@router.get("/")
def list_photos(session: SessionDep) -> Any:
    """
    List public photos.
    """
    return session.query(Photo).all()


@router.post("/")
def upload_photo(session: SessionDep, current_user: CurrentUser, file: UploadFile = File(...)) -> Any:
    """
    Upload a photo.
    """
    photo = Photo()
    session.add(photo)
    return photo


@router.delete("/{id}")
def delete_photo(session: SessionDep, current_user: CurrentUser, id: uuid.UUID) -> Any:
    """
    Delete a photo.
    """
    photo = session.get(Photo, id)
    session.delete(photo)
    return {"ok": True}
