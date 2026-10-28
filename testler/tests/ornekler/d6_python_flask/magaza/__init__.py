from flask import Flask

from .eklentiler import db, jwt, login_manager


def create_app():
    app = Flask(__name__)
    db.init_app(app)
    jwt.init_app(app)
    login_manager.init_app(app)
    from .urunler import bp as urunler_bp

    app.register_blueprint(urunler_bp, url_prefix="/api/urunler")
    from .hesap.views import hesap

    app.register_blueprint(hesap)
    bloklari_kur(app)
    if app.config.get("YONETIM"):
        from .yonetim import yonetim_bp

        app.register_blueprint(yonetim_bp, url_prefix="/yonetim")

    @app.route("/saglik")
    def saglik():
        return {"durum": "ok"}

    return app


def bloklari_kur(app):
    from .api import api_bp

    app.register_blueprint(api_bp)
