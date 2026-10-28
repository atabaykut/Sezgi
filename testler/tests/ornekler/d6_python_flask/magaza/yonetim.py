from flask.views import MethodView
from flask_login import login_required
from flask_smorest import Blueprint

from .modeller import Kategori
from .semalar import KategoriSema

yonetim_bp = Blueprint("yonetim", __name__, url_prefix="/admin")


@yonetim_bp.before_request
@login_required
def yonetim_girisi():
    pass


@yonetim_bp.route("/kategoriler")
class Kategoriler(MethodView):
    @yonetim_bp.response(200, KategoriSema(many=True))
    def get(self):
        return Kategori.query.all()

    @yonetim_bp.arguments(KategoriSema)
    @yonetim_bp.response(201, KategoriSema)
    def post(self, veri):
        return Kategori(**veri)
