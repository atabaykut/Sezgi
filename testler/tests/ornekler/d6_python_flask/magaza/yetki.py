from functools import wraps

from flask import abort
from flask_login import current_user


def yonetici_gerekli(f):
    @wraps(f)
    def sarici(*args, **kwargs):
        if not current_user.is_authenticated or not current_user.is_admin:
            abort(403)
        return f(*args, **kwargs)

    return sarici
