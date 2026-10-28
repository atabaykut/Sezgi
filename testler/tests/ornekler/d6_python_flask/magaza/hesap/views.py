from flask import Blueprint, request
from flask.views import MethodView
from flask_login import login_required

hesap = Blueprint("hesap", __name__, url_prefix="/hesap")


class ProfilView(MethodView):
    decorators = [login_required]

    def get(self):
        return {}

    def put(self):
        data = request.get_json()
        ad = data["ad"]
        telefon = data.get("telefon")
        return {"ad": ad, "telefon": telefon}


hesap.add_url_rule("/profil", view_func=ProfilView.as_view("profil"))


@hesap.route("/giris", methods=["GET", "POST"])
def giris():
    if request.method == "POST":
        eposta = request.form["eposta"]
        return {"eposta": eposta}
    return {}
