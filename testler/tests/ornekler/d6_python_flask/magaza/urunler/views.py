from flask import jsonify, request
from flask_jwt_extended import jwt_required

from ..eklentiler import db
from ..modeller import Urun
from ..semalar import urun_sema
from ..yetki import yonetici_gerekli
from . import bp


@bp.route("/", methods=["GET"])
def listele():
    """Ürünleri listeler"""
    q = request.args.get("q")
    return jsonify([u.ad for u in Urun.query.filter(Urun.ad.contains(q)).all()])


@bp.route("/", methods=["POST"])
@jwt_required()
def olustur():
    veri = urun_sema.load(request.get_json())
    u = Urun(**veri)
    db.session.add(u)
    db.session.commit()
    return urun_sema.dump(u), 201


@bp.get("/<int:urun_id>")
@jwt_required(optional=True)
def getir(urun_id):
    u = db.get_or_404(Urun, urun_id)
    return urun_sema.dump(u)


@bp.delete("/<int:urun_id>")
@yonetici_gerekli
def sil(urun_id):
    u = Urun.query.get_or_404(urun_id)
    db.session.delete(u)
    db.session.commit()
    return "", 204
