from flask import Blueprint
from flask_jwt_extended import jwt_required
from flask_restful import Api, Resource, reqparse

from .eklentiler import db
from .modeller import Siparis

api_bp = Blueprint("api", __name__, url_prefix="/api/v2")
api = Api(api_bp)

parser = reqparse.RequestParser()
parser.add_argument("adet", type=int, required=True, location="json")
parser.add_argument("not", type=str)


class SiparisKaynagi(Resource):
    method_decorators = [jwt_required()]

    def get(self, siparis_id):
        return Siparis.query.get(siparis_id)

    def delete(self, siparis_id):
        s = Siparis.query.get(siparis_id)
        db.session.delete(s)
        return "", 204


class SiparisListesi(Resource):
    def post(self):
        args = parser.parse_args()
        s = Siparis(adet=args["adet"])
        db.session.add(s)
        return {}, 201


api.add_resource(SiparisListesi, "/siparisler")
api.add_resource(SiparisKaynagi, "/siparisler/<int:siparis_id>")
