// Yerel depo: iptal kuralı burada (kaynak kodu yokmuş gibi incelenir)
const kayitlar = new Map();
exports.iptalEt = (id) => {
  const r = kayitlar.get(id);
  if (!r) return { tamam: false };
  r.durum = 'iptal';
  return { tamam: true };
};
exports.liste = () => [...kayitlar.values()];
exports.gunluk = (satir) => console.log(satir);
