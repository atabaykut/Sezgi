from marshmallow import Schema, fields, validate


class UrunSema(Schema):
    id = fields.Int(dump_only=True)
    ad = fields.Str(required=True, validate=validate.Length(min=2, max=120))
    fiyat = fields.Float(required=True, validate=validate.Range(min=0))
    kategori = fields.Str(validate=validate.OneOf(["kitap", "oyun"]))
    notlar = fields.Str(load_only=True)


class KategoriSema(Schema):
    id = fields.Int(dump_only=True)
    ad = fields.Str(required=True)


urun_sema = UrunSema()
