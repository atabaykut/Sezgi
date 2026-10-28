import axios from 'axios';

export async function kitaplariGetir() {
  // Birebir adres: kesin
  const r = await fetch('/api/v1/kitaplar');
  return r.json();
}

export async function kitapEkle(kitap: unknown) {
  return axios.post(`/api/v1/kitaplar`, kitap);
}

export async function kitapSil(id: string) {
  // Şablon: yol parametresi yerine değişken, adres birebir: kesin
  return axios.delete(`/api/v1/kitaplar/${id}`);
}

export async function onekli(api: { get: (u: string) => Promise<unknown> }) {
  // Genel önek istemcinin taban adresinde: tahmin (yorum)
  return api.get('/kitaplar');
}

export async function olmayan() {
  return fetch('/api/v1/boyle-bir-yer-yok');
}
