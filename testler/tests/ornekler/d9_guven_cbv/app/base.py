from fastapi import Depends

from app.deps import User, get_current_user, get_db


class BaseController:
    session = Depends(get_db)


class BaseUserController(BaseController):
    user: User = Depends(get_current_user)
