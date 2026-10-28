from .eklentiler import db

Model = db.Model


class Kategori(Model):
    id = db.Column(db.Integer, primary_key=True)
    ad = db.Column(db.String(50), nullable=False)
    urunler = db.relationship("Urun", cascade="all, delete-orphan")


class Urun(Model):
    __tablename__ = "urunler"
    id = db.Column(db.Integer, primary_key=True)
    ad = db.Column(db.String(120), nullable=False)
    fiyat = db.Column(db.Float, nullable=False)
    kategori_id = db.Column(db.Integer, db.ForeignKey("kategori.id", ondelete="CASCADE"))


class Siparis(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    urun_id = db.Column(db.Integer, db.ForeignKey("urunler.id", ondelete="CASCADE"))
    adet = db.Column(db.Integer, nullable=False)
