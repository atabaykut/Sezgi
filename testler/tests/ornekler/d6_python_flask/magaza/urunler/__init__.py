from flask import Blueprint

bp = Blueprint("urunler", __name__, url_prefix="/eski")

from . import views  # noqa
