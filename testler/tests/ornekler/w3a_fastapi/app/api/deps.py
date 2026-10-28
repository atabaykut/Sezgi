from fastapi import HTTPException


def admin_iste(token: str = "") -> None:
    if token != "admin":
        raise HTTPException(status_code=403, detail="Admin required")
