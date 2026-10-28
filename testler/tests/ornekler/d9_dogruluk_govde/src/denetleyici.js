const kullanicilar = new Map();

const kayit = async (req, res) => {
  const { ad, eposta } = req.body;
  kullanicilar.set(eposta, { ad, eposta });
  res.status(201).json({ ad, eposta });
};

const guncelle = async (req, res) => {
  const k = kullanicilar.get(req.params.id);
  k.ad = req.body.ad || k.ad;
  k.eposta = req.body.eposta || k.eposta;
  res.json(k);
};

export { kayit, guncelle };
