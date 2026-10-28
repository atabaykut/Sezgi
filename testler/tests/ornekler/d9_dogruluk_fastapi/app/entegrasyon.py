from fastapi import HTTPException


def save(request: dict, rw: bool) -> None:
    if not rw:
        raise HTTPException(status_code=403, detail="read-only member")
