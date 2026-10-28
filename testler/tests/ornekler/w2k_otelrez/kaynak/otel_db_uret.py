# otel.db şemasını üretir (yayin/otel.db). Kaynağın parçası: yayımlanmış klasöre yalnızca DB gider.
import os
import sqlite3
import sys

yol = sys.argv[1]
if os.path.exists(yol):
    os.remove(yol)
db = sqlite3.connect(yol)
db.executescript("""
CREATE TABLE oda (
  no INTEGER PRIMARY KEY,
  tip TEXT NOT NULL CHECK (tip IN ('tek', 'cift', 'suit')),
  fiyat NUMERIC NOT NULL CHECK (fiyat > 0)
);
CREATE TABLE misafir (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ad TEXT NOT NULL,
  eposta TEXT UNIQUE
);
CREATE TABLE "rezervasyon" (
  id INTEGER PRIMARY KEY,
  oda_no INTEGER NOT NULL REFERENCES oda(no) ON DELETE RESTRICT,
  misafir_id INTEGER NOT NULL,
  giris TEXT NOT NULL,
  tutar NUMERIC NOT NULL DEFAULT 0,
  durum TEXT NOT NULL DEFAULT 'aktif' CHECK (durum IN ('aktif', 'iptal')), -- iptal kuralı uygulamada
  /* sahip kontrolü uygulamada */
  CONSTRAINT fk_misafir FOREIGN KEY (misafir_id) REFERENCES misafir (id),
  CHECK (tutar >= 0)
);
CREATE INDEX ix_rez_oda ON rezervasyon (oda_no, giris);
CREATE UNIQUE INDEX ux_misafir_eposta ON misafir (eposta);
CREATE TABLE iptal_kaydi (rez_id INTEGER NOT NULL REFERENCES rezervasyon(id), zaman TEXT NOT NULL);
CREATE TRIGGER trg_iptal AFTER UPDATE OF durum ON rezervasyon
WHEN NEW.durum = 'iptal'
BEGIN
  INSERT INTO iptal_kaydi (rez_id, zaman) VALUES (NEW.id, datetime('now'));
END;
CREATE VIEW aktif_rezervasyon AS SELECT * FROM rezervasyon WHERE durum = 'aktif';
""")
# Şema kayıtlarının birden fazla sayfaya ve taşma sayfasına yayılması için uzun yorumlu tablolar
for i in range(40):
    db.execute(f"CREATE TABLE ek_{i:02d} (id INTEGER PRIMARY KEY, deger TEXT /* {'x' * (300 + i * 40)} */)")
db.execute("INSERT INTO oda VALUES (101, 'tek', 1000)")
db.commit()
db.close()
