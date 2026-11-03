exports.ozet = (req, res) => {
  const sayfa = req.query.sayfa;
  return db.prepare('SELECT count(*) FROM kitaplar').get();
};
